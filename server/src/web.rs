//! Интерфейс (SvelteKit, SPA) внутри программы и заголовки безопасности.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

#[derive(rust_embed::Embed)]
#[folder = "../web/build"]
struct Assets;

pub async fn spa(uri: Uri) -> Response {
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
    Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cache)
        .body(Body::from(file.data.into_owned()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Внешнее — только для видео по ссылкам: плееры Rutube, VK и YouTube во фрейме, прямые
/// mp4/m3u8 с других сайтов. Всё остальное интерфейс берёт только у нас.
const CSP: &str =
    "default-src 'self'; img-src 'self' data: blob:; media-src 'self' blob: https:; style-src 'self' 'unsafe-inline'; \
script-src 'self' 'unsafe-inline'; connect-src 'self' https:; font-src 'self'; \
frame-src https://rutube.ru https://vkvideo.ru https://vk.com https://www.youtube-nocookie.com; \
frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

/// Заголовки на все ответы. `X-Fsfy: 1` — метка «ответил наш сервер», а не страница туннеля:
/// без неё интерфейс считает, что компьютер хоста выключен.
pub async fn headers(req: Request, next: Next) -> Response {
    // Защита от подделки запросов: изменения — только с заголовком, который чужой сайт без CORS
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
    h.insert("permissions-policy", HeaderValue::from_static("camera=(), microphone=(), geolocation=()"));
    if !h.contains_key(header::CONTENT_SECURITY_POLICY) {
        h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    }
    res
}
