//! Интерфейс (SvelteKit, SPA) внутри программы и заголовки безопасности.

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

#[derive(rust_embed::Embed)]
#[folder = "../web/build"]
struct Assets;

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
    let response = Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cache)
        .header(header::ETAG, etag);
    if unchanged {
        return response
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }
    let bytes = match file.data {
        std::borrow::Cow::Borrowed(data) => Bytes::from_static(data),
        std::borrow::Cow::Owned(data) => Bytes::from(data),
    };
    response.body(Body::from(bytes)).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
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
