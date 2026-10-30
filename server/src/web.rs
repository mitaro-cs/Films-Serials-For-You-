//! Интерфейс (SvelteKit, SPA) внутри программы и заголовки безопасности.

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

#[derive(rust_embed::Embed)]
#[folder = "../web/build"]
struct Assets;

/// Сжатые копии неизменяемых файлов интерфейса (`_app/immutable/…` — имя меняется с содержимым):
/// сжимаем один раз и сильно (brotli 11, gzip 9), а не при каждом запросе — хосту меньше работы,
/// через туннель — меньше байт.
static PACKED: std::sync::LazyLock<parking_lot::Mutex<std::collections::HashMap<(String, bool), bytes::Bytes>>> =
    std::sync::LazyLock::new(Default::default);

fn pack(name: &str, data: &[u8], br: bool) -> bytes::Bytes {
    use std::io::Write;
    let key = (name.to_string(), br);
    if let Some(b) = PACKED.lock().get(&key) {
        return b.clone();
    }
    let mut out = Vec::with_capacity(data.len() / 3);
    if br {
        // Large lazy-loaded player bundles must not hold the first playback request
        // for seconds while Brotli performs its most expensive compression pass.
        let params = brotli::enc::BrotliEncoderParams { quality: 5, lgwin: 22, ..Default::default() };
        let _ = brotli::BrotliCompress(&mut &data[..], &mut out, &params);
    } else {
        let mut z = flate2::write::GzEncoder::new(&mut out, flate2::Compression::best());
        let _ = z.write_all(data).and_then(|_| z.try_finish());
        drop(z);
    }
    let b = bytes::Bytes::from(out);
    PACKED.lock().insert(key, b.clone());
    b
}

/// Сжать всё неизменяемое заранее (при запуске, в фоне): первый зритель не ждёт.
pub fn warm() {
    for name in Assets::iter().filter(|n| n.starts_with("_app/immutable/") && !n.ends_with(".png")) {
        if let Some(f) = Assets::get(&name).filter(|f| f.data.len() > 1024) {
            pack(&name, &f.data, true);
            pack(&name, &f.data, false);
        }
    }
}

/// Какое сжатие понимает браузер: brotli лучше, иначе gzip.
fn accepts(headers: &axum::http::HeaderMap) -> Option<bool> {
    let v = headers.get(header::ACCEPT_ENCODING)?.to_str().ok()?;
    let has = |n: &str| v.split(',').any(|p| p.split(';').next().is_some_and(|x| x.trim() == n));
    if has("br") {
        Some(true)
    } else if has("gzip") {
        Some(false)
    } else {
        None
    }
}

pub async fn spa(uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") {
        return crate::error::ApiError::not_found().into_response();
    }
    let (file, name) = match Assets::get(path).filter(|_| !path.is_empty()) {
        Some(f) => (f, path),
        None => match Assets::get("index.html") {
            Some(f) => (f, "index.html"),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    let mime = mime_guess::from_path(name).first_or_octet_stream();
    let cache = if name.starts_with("_app/immutable/") {
        "public, max-age=31536000, immutable"
    } else if name == "index.html" {
        "no-cache"
    } else {
        "public, max-age=3600"
    };
    let etag = format!("\"{}\"", hex::encode(file.metadata.sha256_hash()));
    let unchanged = headers.get(header::IF_NONE_MATCH).and_then(|h| h.to_str().ok()).is_some_and(|h| {
        h.split(',').any(|v| {
            let v = v.trim();
            v == "*" || v.strip_prefix("W/").unwrap_or(v) == etag
        })
    });
    let res = Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cache)
        .header(header::ETAG, etag)
        .header(header::VARY, "accept-encoding");
    if unchanged {
        return res
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }
    let packable =
        name.starts_with("_app/immutable/") && file.data.len() > 1024 && !mime.type_().eq(&mime_guess::mime::IMAGE);
    if let Some(br) = accepts(&headers).filter(|_| packable) {
        let data = file.data.clone();
        let name = name.to_string();
        let packed = tokio::task::spawn_blocking(move || pack(&name, &data, br)).await.unwrap_or_default();
        if !packed.is_empty() {
            return res
                .header(header::CONTENT_ENCODING, if br { "br" } else { "gzip" })
                .body(Body::from(packed))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    }
    let bytes = match file.data {
        std::borrow::Cow::Borrowed(data) => Bytes::from_static(data),
        std::borrow::Cow::Owned(data) => Bytes::from(data),
    };
    res.body(Body::from(bytes)).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Внешнее - только для видео по ссылкам: плееры Rutube, VK и YouTube во фрейме, прямые
/// mp4/m3u8 с других сайтов. Всё остальное интерфейс берёт только у нас.
const CSP: &str =
    "default-src 'self'; img-src 'self' data: blob:; media-src 'self' blob: https:; style-src 'self' 'unsafe-inline'; \
script-src 'self' 'unsafe-inline'; connect-src 'self' https:; font-src 'self'; \
frame-src https://rutube.ru https://vkvideo.ru https://vk.com https://www.youtube-nocookie.com; \
frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

/// Заголовки на все ответы. `X-Fsfy: 1` - метка «ответил наш сервер», а не страница туннеля:
/// без неё интерфейс считает, что компьютер хоста выключен.
pub async fn headers(req: Request, next: Next) -> Response {
    // Защита от подделки запросов: изменения - только с заголовком, который чужой сайт без CORS
    // поставить не может.
    let unsafe_method = !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if unsafe_method && req.uri().path().starts_with("/api/") && req.headers().get("x-fsfy").is_none_or(|v| v != "1") {
        return crate::error::ApiError::new(StatusCode::FORBIDDEN, "csrf", "Запрос отклонён: обновите страницу")
            .into_response();
    }
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-fsfy", HeaderValue::from_static("1"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("same-origin"));
    // Микрофон - только самому сайту (голосовой чат в совместном просмотре).
    h.insert("permissions-policy", HeaderValue::from_static("camera=(), microphone=(self), geolocation=()"));
    if !h.contains_key(header::CONTENT_SECURITY_POLICY) {
        h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cached_shell_revalidates_without_transferring_body() {
        let uri: Uri = "/".parse().unwrap();
        let first = spa(uri.clone(), HeaderMap::new()).await;
        assert_eq!(first.status(), StatusCode::OK);
        let etag = first.headers()[header::ETAG].to_str().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, format!("\"old\", W/{etag}").parse().unwrap());
        let cached = spa(uri.clone(), headers).await;
        assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
        assert!(axum::body::to_bytes(cached.into_body(), 1024).await.unwrap().is_empty());
        let mut stale = HeaderMap::new();
        stale.insert(header::IF_NONE_MATCH, HeaderValue::from_static("\"old\""));
        assert_eq!(spa(uri, stale).await.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod pack_tests {
    use super::*;

    #[test]
    fn picks_encoding_and_round_trips() {
        let mut h = axum::http::HeaderMap::new();
        assert_eq!(accepts(&h), None);
        h.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate, br;q=1.0, zstd"));
        assert_eq!(accepts(&h), Some(true));
        h.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("gzip"));
        assert_eq!(accepts(&h), Some(false));
        h.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("brx, identity"));
        assert_eq!(accepts(&h), None);

        let data = "let x = 1; ".repeat(500);
        let gz = pack("_app/immutable/test.js", data.as_bytes(), false);
        let mut out = String::new();
        std::io::Read::read_to_string(&mut flate2::read::GzDecoder::new(&gz[..]), &mut out).unwrap();
        assert_eq!(out, data);
        let br = pack("_app/immutable/test.js", data.as_bytes(), true);
        let mut out = Vec::new();
        brotli::BrotliDecompress(&mut &br[..], &mut out).unwrap();
        assert_eq!(out, data.as_bytes());
        assert!(br.len() < data.len() / 10);
    }
}
