//! Видео Rutube и VK Видео - в нашем плеере. Сервер находит поток сайта (HLS) и отдаёт его через
//! себя: браузеру сайты свой поток напрямую не дают (Rutube - без CORS, VK - ссылка привязана к
//! адресу компьютера хоста). Плейлисты переписываются на `p?u=…`, кусочки идут потоком.
//! Ходим только на известные адреса сайтов - посредником «куда угодно» сервер не станет.

use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::util::now_ms;
use crate::App;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use parking_lot::Mutex;
use reqwest::Url;
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Как обычный браузер: иначе сайты отдают страницу-заглушку.
const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0 Safari/537.36";
/// Ссылки на поток у сайтов живут несколько часов - помним 20 минут.
const KEEP_MS: i64 = 20 * 60_000;

const RUTUBE: &[&str] = &["rutube.ru", "rtbcdn.ru", "rutubelist.ru"];
const VK: &[&str] = &["okcdn.ru", "vkuser.net", "userapi.com", "mycdn.me", "vkvideo.ru", "vk.com", "vk-cdn.net"];

fn hosts(kind: &str) -> &'static [&'static str] {
    match kind {
        "rutube" => RUTUBE,
        "vk" => VK,
        _ => &[],
    }
}

/// Адрес с сайта этой ссылки (и только https).
pub fn allowed(kind: &str, u: &Url) -> bool {
    let Some(h) = u.host_str().map(str::to_lowercase) else { return false };
    u.scheme() == "https" && hosts(kind).iter().any(|a| h == *a || h.ends_with(&format!(".{a}")))
}

/// Ссылка → (главный плейлист или файл, когда узнали, какие ещё адреса можно (свой сайт)).
type Cache = Mutex<HashMap<String, (String, i64, Vec<String>)>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(Mutex::default)
}

fn client() -> reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        crate::outbound::builder()
            .user_agent(UA)
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("public video client")
    })
    .clone()
}

/// GET с переходами и куками (VK по дороге ставит куки и переадресует к себе же).
pub async fn page(url: &str) -> anyhow::Result<String> {
    Ok(page_with_url(url).await?.1)
}

pub async fn page_with_url(url: &str) -> anyhow::Result<(Url, String)> {
    let mut url = Url::parse(url)?;
    let mut cookies: HashMap<String, String> = HashMap::new();
    for _ in 0..10 {
        anyhow::ensure!(crate::outbound::valid_url(&url), "внутренний адрес запрещён");
        let jar = cookies.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ");
        let r = client().get(url.clone()).header(header::COOKIE, jar).timeout(Duration::from_secs(12)).send().await?;
        for c in r.headers().get_all(header::SET_COOKIE) {
            let c = c.to_str().unwrap_or("");
            let kv = c.split(';').next().unwrap_or("");
            if let Some((k, v)) = kv.split_once('=') {
                cookies.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        if r.status().is_redirection() {
            let loc = r.headers().get(header::LOCATION).and_then(|l| l.to_str().ok()).unwrap_or("");
            let next = url.join(loc)?;
            if next.origin() != url.origin() {
                cookies.clear();
            }
            url = next;
            continue;
        }
        anyhow::ensure!(r.status().is_success(), "сайт ответил {}", r.status().as_u16());
        return Ok((url, String::from_utf8_lossy(&crate::outbound::limited(r, 4 * 1024 * 1024).await?).into_owned()));
    }
    anyhow::bail!("слишком много переадресаций")
}

/// Строка JSON из страницы: `"key":"…"` → значение (с разбором `\/` и `&`).
pub fn json_str(page: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":\"");
    let at = page.find(&pat)? + pat.len();
    let rest = &page[at..];
    let b = rest.as_bytes();
    let mut end = 0;
    while end < b.len() {
        match b[end] {
            b'\\' => end += 2,
            b'"' => break,
            _ => end += 1,
        }
    }
    serde_json::from_str::<String>(&format!("\"{}\"", rest.get(..end)?)).ok().filter(|s| !s.is_empty())
}

/// Адрес видео на странице своего сайта: сначала поток HLS (.m3u8), потом файл (.mp4, .webm).
pub fn find_media(page: &str) -> Option<String> {
    let text = page.replace("\\/", "/").replace("\\u0026", "&").replace("&amp;", "&");
    let re =
        regex::Regex::new(r#"(https?://[^\s"'<>\\]+\.(m3u8|mp4|webm)(?:\?[^\s"'<>\\]*)?)(?:[\s"'<>\\]|$)"#).ok()?;
    let found: Vec<(String, bool)> = re.captures_iter(&text).map(|c| (c[1].to_string(), &c[2] == "m3u8")).collect();
    found.iter().max_by_key(|f| (f.1, super::links::quality_hint(&f.0))).map(|f| f.0.clone())
}

/// Первый встроенный плеер страницы (`<iframe src>`): видео часто лежит в нём.
#[cfg(test)]
pub fn find_iframe(page: &str, base: &Url) -> Option<Url> {
    find_iframes(page, base).into_iter().next()
}

pub fn find_iframes(page: &str, base: &Url) -> Vec<Url> {
    let Ok(re) = regex::Regex::new(r#"(?i)<iframe[^>]+(?:src|data-src)\s*=\s*(?:["']([^"']+)["']|([^\s>"']+))"#) else {
        return vec![];
    };
    re.captures_iter(page)
        .filter_map(|c| base.join(&c.get(1).or_else(|| c.get(2))?.as_str().trim().replace("&amp;", "&")).ok())
        .filter(|u| u.scheme() == "https" || u.scheme() == "http")
        .filter(|u| {
            !u.path().to_ascii_lowercase().split('/').any(|part| matches!(part, "trailer" | "trailers" | "teaser"))
        })
        .take(8)
        .collect()
}

/// «cdn.kino.example» → «kino.example»: кусочки у сайтов часто на соседних поддоменах.
fn base_domain(u: &Url) -> Option<String> {
    let h = u.host_str()?.to_lowercase();
    let parts: Vec<&str> = h.split('.').collect();
    (parts.len() >= 2).then(|| parts[parts.len() - 2..].join("."))
}

/// Адрес с сайта этой ссылки или с адресов, которые нашлись у своего сайта.
fn allowed_for(kind: &str, u: &Url, extra: &[String]) -> bool {
    if allowed(kind, u) {
        return true;
    }
    let Some(h) = u.host_str().map(str::to_lowercase) else { return false };
    matches!(kind, "site" | "direct")
        && matches!(u.scheme(), "https" | "http")
        && extra.iter().any(|a| h == *a || h.ends_with(&format!(".{a}")))
}

/// Главный плейлист (или файл) видео у сайта и какие ещё адреса можно (свой сайт).
async fn master(kind: &str, id: &str, url: &str, embed: Option<&str>) -> anyhow::Result<(String, Vec<String>)> {
    let found = match kind {
        "rutube" => {
            let v: serde_json::Value = client()
                .get(format!("https://rutube.ru/api/play/options/{id}/?no_404=true&referer=https%3A%2F%2Frutube.ru"))
                .timeout(Duration::from_secs(10))
                .send()
                .await?
                .json()
                .await?;
            let m = v["video_balancer"]["m3u8"]
                .as_str()
                .or(v["video_balancer"]["default"].as_str())
                .map(String::from)
                .ok_or_else(|| anyhow::anyhow!("Rutube не отдал видео (закрыто или удалено)"))?;
            (m, vec![])
        }
        "vk" => {
            let p = page(embed.ok_or_else(|| anyhow::anyhow!("нет ссылки VK"))?).await?;
            let m = json_str(&p, "hls")
                .or_else(|| json_str(&p, "hls_ondemand"))
                .ok_or_else(|| anyhow::anyhow!("VK не отдал видео (закрыто, удалено или только для своих)"))?;
            (m, vec![])
        }
        "site" if Url::parse(url)?.host_str() == Some("www.dailymotion.com") => {
            let v: Value = client()
                .get(format!("https://www.dailymotion.com/player/metadata/video/{id}"))
                .timeout(Duration::from_secs(6))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let stream = v["qualities"]
                .as_object()
                .into_iter()
                .flatten()
                .flat_map(|(_, v)| v.as_array().into_iter().flatten())
                .filter(|v| v["type"].as_str().is_some_and(|t| t.contains("mpegURL") || t.contains("mpegurl")))
                .find_map(|v| v["url"].as_str())
                .ok_or_else(|| anyhow::anyhow!("Dailymotion не предоставляет доступный HLS-поток"))?;
            (stream.to_string(), vec!["dailymotion.com".into(), "dmcdn.net".into()])
        }
        "site" if crate::lordfilm::series_url(&Url::parse(url)?) => {
            let (media, _) = crate::lordfilm::episode_stream(&Url::parse(url)?).await?;
            let extra = base_domain(&Url::parse(&media)?).into_iter().collect();
            (media, extra)
        }
        "site" => {
            let base = Url::parse(url)?;
            let mut extra: Vec<String> = base_domain(&base).into_iter().collect();
            let p = page(url).await?;
            let mut media = find_media(&p);
            let mut queue: std::collections::VecDeque<(Url, usize)> =
                find_iframes(&p, &base).into_iter().map(|u| (u, 1)).collect();
            let mut visited = std::collections::HashSet::new();
            while media.is_none() && visited.len() < 8 {
                let Some((frame, depth)) = queue.pop_front() else { break };
                if !visited.insert(frame.to_string()) {
                    continue;
                }
                // Не останавливаемся на рекламном/недоступном iframe: пробуем следующий плеер.
                let Ok(html) = page(frame.as_str()).await else { continue };
                extra.extend(base_domain(&frame));
                media = find_media(&html);
                if depth < 3 {
                    queue.extend(find_iframes(&html, &frame).into_iter().map(|u| (u, depth + 1)));
                }
            }
            let media = media.ok_or_else(|| anyhow::anyhow!("на странице не нашлось видео - откройте его на сайте"))?;
            extra.extend(Url::parse(&media).ok().as_ref().and_then(base_domain));
            extra.dedup();
            (media, extra)
        }
        _ => anyhow::bail!("этот сайт играет только своим плеером"),
    };
    let u = Url::parse(&found.0)?;
    anyhow::ensure!(allowed_for(kind, &u, &found.1) && crate::outbound::valid_url(&u), "неожиданный адрес видео");
    Ok(found)
}

/// Плейлист: адреса кусочков и вложенных плейлистов - через нас (`p?u=`).
pub fn rewrite(text: &str, base: &Url) -> String {
    let wrap = |s: &str| -> String {
        match base.join(s.trim()) {
            Ok(u) => format!("p?u={}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(u.as_str())),
            Err(_) => s.to_string(),
        }
    };
    let mut out = String::with_capacity(text.len() + 1024);
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            out.push('\n');
            continue;
        }
        if t.starts_with('#') {
            // URI="…" в EXT-X-KEY, EXT-X-MAP, EXT-X-MEDIA.
            if let Some(i) = t.find("URI=\"") {
                let s = i + 5;
                if let Some(e) = t[s..].find('"') {
                    out.push_str(&t[..s]);
                    out.push_str(&wrap(&t[s..s + e]));
                    out.push_str(&t[s + e..]);
                    out.push('\n');
                    continue;
                }
            }
            out.push_str(t);
        } else {
            out.push_str(&wrap(t));
        }
        out.push('\n');
    }
    out
}

type LinkRow = (String, String, Option<String>);

fn link_of(app: &App, id: i64) -> ApiResult<LinkRow> {
    let row: Option<LinkRow> = app.db.with(|c| {
        c.query_row("SELECT kind, url, embed FROM links WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
    })?;
    row.ok_or_else(ApiError::not_found)
}

fn playlist(body: String) -> Response {
    ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl"), (header::CACHE_CONTROL, "no-cache")], body)
        .into_response()
}

fn failed(e: impl std::fmt::Display) -> ApiError {
    ApiError::new(StatusCode::BAD_GATEWAY, "site", format!("Видео с сайта не открылось: {e}"))
}

fn b64(u: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(u)
}

/// Видео ссылки у сайта (из памяти на 20 минут): вид, адрес, какие ещё адреса можно.
async fn resolve(app: &App, id: i64) -> ApiResult<(String, String, Vec<String>)> {
    let (kind, url, embed) = link_of(app, id)?;
    if kind == "direct" {
        let parsed = Url::parse(&url).map_err(failed)?;
        if !crate::outbound::valid_url(&parsed) {
            return Err(ApiError::bad("source", "Внутренний адрес запрещён"));
        }
        let host = parsed.host_str().ok_or_else(ApiError::not_found)?.to_lowercase();
        return Ok((kind, url, vec![host]));
    }
    if hosts(&kind).is_empty() && kind != "site" {
        return Err(ApiError::bad("own_player", "Этот сайт играет только своим плеером"));
    }
    if let Some(c) = cache().lock().get(&url).filter(|c| now_ms() - c.1 < KEEP_MS) {
        return Ok((kind, c.0.clone(), c.2.clone()));
    }
    let sid = super::links::parse(&url).and_then(|p| p.id).unwrap_or_default();
    let (m, extra) = master(&kind, &sid, &url, embed.as_deref()).await.map_err(failed)?;
    cache().lock().insert(url, (m.clone(), now_ms(), extra.clone()));
    Ok((kind, m, extra))
}

fn invalidate(app: &App, id: i64) {
    if let Ok((_, url, _)) = link_of(app, id) {
        cache().lock().remove(&url);
    }
}

fn is_playlist(u: &Url) -> bool {
    u.path().to_lowercase().ends_with(".m3u8")
}

/// Переходы CDN проверяются на каждом шаге, Range сохраняется.
async fn fetch_media(
    kind: &str,
    mut url: Url,
    extra: &[String],
    range: Option<&axum::http::HeaderValue>,
) -> anyhow::Result<(Url, reqwest::Response)> {
    for _ in 0..5 {
        anyhow::ensure!(
            allowed_for(kind, &url, extra) && crate::outbound::valid_url(&url),
            "неразрешённый адрес потока"
        );
        let mut request = client()
            .get(url.clone())
            .header(
                header::REFERER,
                if kind == "vk" {
                    "https://vkvideo.ru/"
                } else if extra.iter().any(|h| h == "dailymotion.com") {
                    "https://www.dailymotion.com/"
                } else {
                    "https://rutube.ru/"
                },
            )
            .timeout(Duration::from_secs(30));
        if let Some(range) = range {
            request = request.header(header::RANGE, range.clone());
        }
        let response = request.send().await?;
        if response.status().is_redirection() {
            let next = response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| anyhow::anyhow!("нет адреса перехода CDN"))?;
            url = url.join(next)?;
            continue;
        }
        anyhow::ensure!(response.status().is_success(), "CDN ответил {}", response.status());
        return Ok((url, response));
    }
    anyhow::bail!("слишком много переходов CDN")
}

/// Что играть плееру: поток HLS через нас или файл через нас (с перемоткой).
pub async fn source(State(app): State<Arc<App>>, _v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let (kind, m, _) = resolve(&app, id).await?;
    let u = Url::parse(&m).map_err(failed)?;
    Ok(Json(if !matches!(kind.as_str(), "site" | "direct") || is_playlist(&u) {
        json!({"url": format!("/api/links/{id}/stream/index.m3u8"), "hls": true})
    } else {
        json!({"url": format!("/api/links/{id}/stream/p?u={}", b64(&m)), "hls": false})
    }))
}

/// Главный плейлист видео по ссылке - для нашего плеера.
#[derive(Deserialize)]
pub struct QualityQuery {
    height: Option<u32>,
}

fn select_quality(text: &str, height: Option<u32>) -> String {
    let Some(height) = height else { return text.to_owned() };
    let lines: Vec<_> = text.lines().collect();
    let wanted = format!("x{height}");
    let mut found = false;
    let mut output = Vec::new();
    let mut n = 0;
    while n < lines.len() {
        if lines[n].starts_with("#EXT-X-STREAM-INF:") {
            let matches = lines[n]
                .split_once("RESOLUTION=")
                .is_some_and(|(_, r)| r.split(',').next().is_some_and(|resolution| resolution.ends_with(&wanted)));
            if matches {
                found = true;
                output.push(lines[n]);
                if n + 1 < lines.len() {
                    output.push(lines[n + 1]);
                }
            }
            n += 2;
        } else {
            output.push(lines[n]);
            n += 1;
        }
    }
    if found {
        output.join("\n")
    } else {
        text.to_owned()
    }
}

pub async fn index(
    State(app): State<Arc<App>>,
    _v: Viewer,
    Path(id): Path<i64>,
    Query(q): Query<QualityQuery>,
) -> ApiResult<Response> {
    let (kind, m, extra) = resolve(&app, id).await?;
    let base = Url::parse(&m).map_err(failed)?;
    let (base, r) = fetch_media(&kind, base, &extra, None).await.map_err(|error| {
        invalidate(&app, id);
        failed(error)
    })?;
    if !r.status().is_success() {
        return Err(failed(format!("сайт ответил {}", r.status().as_u16())));
    }
    let text =
        String::from_utf8(crate::outbound::limited(r, 2 * 1024 * 1024).await.map_err(failed)?).map_err(failed)?;
    Ok(playlist(rewrite(&select_quality(&text, q.height), &base)))
}

#[derive(Deserialize)]
pub struct PartQuery {
    u: String,
}

/// Вложенный плейлист, кусочек видео или файл целиком - с сайта через нас (Range - как есть).
pub async fn part(
    State(app): State<Arc<App>>,
    _v: Viewer,
    Path(id): Path<i64>,
    Query(q): Query<PartQuery>,
    headers: axum::http::HeaderMap,
) -> ApiResult<Response> {
    let (kind, _, extra) = resolve(&app, id).await?;
    let raw =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(q.u.as_bytes()).map_err(|_| ApiError::not_found())?;
    let u = std::str::from_utf8(&raw).ok().and_then(|s| Url::parse(s).ok()).ok_or_else(ApiError::not_found)?;
    if !allowed_for(&kind, &u, &extra) || !crate::outbound::valid_url(&u) {
        return Err(ApiError::not_found());
    }
    let (u, r) = fetch_media(&kind, u, &extra, headers.get(header::RANGE)).await.map_err(|error| {
        invalidate(&app, id);
        failed(error)
    })?;
    let status = r.status();
    if !status.is_success() {
        return Err(failed(format!("сайт ответил {}", status.as_u16())));
    }
    let ct = r.headers().get(header::CONTENT_TYPE).and_then(|c| c.to_str().ok()).unwrap_or("").to_lowercase();
    if ct.contains("mpegurl") || is_playlist(&u) {
        let text =
            String::from_utf8(crate::outbound::limited(r, 2 * 1024 * 1024).await.map_err(failed)?).map_err(failed)?;
        return Ok(playlist(rewrite(&text, &u)));
    }
    let code = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
    let mut resp = Response::builder()
        .status(code)
        .header(header::CONTENT_TYPE, if ct.is_empty() { "video/mp2t".to_string() } else { ct })
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=3600");
    for h in [header::CONTENT_LENGTH, header::CONTENT_RANGE] {
        if let Some(v) = r.headers().get(&h) {
            resp = resp.header(h, v.clone());
        }
    }
    resp.body(Body::from_stream(r.bytes_stream())).map_err(ApiError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_quality_preserves_audio_and_auto() {
        let text = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",URI=\"audio.m3u8\"\n#EXT-X-STREAM-INF:RESOLUTION=1280x720,AUDIO=\"a\"\n720.m3u8\n#EXT-X-STREAM-INF:RESOLUTION=640x360,AUDIO=\"a\"\n360.m3u8\n";
        let selected = select_quality(text, Some(720));
        assert!(selected.contains("#EXT-X-MEDIA:"));
        assert!(selected.contains("720.m3u8"));
        assert!(!selected.contains("360.m3u8"));
        assert_eq!(select_quality(text, None), text);
        assert_eq!(select_quality(text, Some(1080)), text);
    }

    #[test]
    fn rewrites_playlists() {
        let base = Url::parse("https://river.rtbcdn.ru/hls/a/master.m3u8?x=1").unwrap();
        let src =
            "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n#EXTINF:2,\nseg-1.ts\n\nhttps://x.rtbcdn.ru/b.m3u8\n";
        let out = rewrite(src, &base);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "#EXTM3U");
        assert!(lines[1].starts_with("#EXT-X-KEY:METHOD=AES-128,URI=\"p?u=") && lines[1].ends_with('"'));
        let seg = lines[3].strip_prefix("p?u=").unwrap();
        let back = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(seg).unwrap();
        assert_eq!(std::str::from_utf8(&back).unwrap(), "https://river.rtbcdn.ru/hls/a/seg-1.ts");
        assert!(lines[5].starts_with("p?u="));
    }

    #[test]
    fn only_known_sites() {
        let ok = |k: &str, u: &str| allowed(k, &Url::parse(u).unwrap());
        assert!(ok("rutube", "https://river-m9.rtbcdn.ru/x.ts"));
        assert!(ok("vk", "https://vkvd743.okcdn.ru/video.m3u8"));
        assert!(!ok("rutube", "https://evil.example/x.ts"));
        assert!(!ok("rutube", "https://rtbcdn.ru.evil.example/x.ts"));
        assert!(!ok("vk", "http://vkvd743.okcdn.ru/video.m3u8"));
        assert!(!ok("youtube", "https://www.youtube.com/"));
    }

    #[test]
    fn json_strings_from_page() {
        let page = r#"var p = {"hls":"https:\/\/vkvd1.okcdn.ru\/video.m3u8?a=1&b=2","x":"y"};"#;
        assert_eq!(json_str(page, "hls").as_deref(), Some("https://vkvd1.okcdn.ru/video.m3u8?a=1&b=2"));
        assert_eq!(json_str(page, "none"), None);
    }

    #[test]
    fn media_on_site_pages() {
        assert_eq!(
            find_media(r#"hls: "https://cdn.example/movie.mp4/master.m3u8?token=abc""#).as_deref(),
            Some("https://cdn.example/movie.mp4/master.m3u8?token=abc")
        );
        assert_eq!(find_media(r#"src="https://cdn.example/movie.mp4/thumbnail.jpg""#), None);
        let p = r#"<video src="https://cdn.kino.example/a/film.mp4?t=1"></video><script>var f={"hls":"https:\/\/s2.kino.example\/v\/master.m3u8?k=1&x=2"}</script>"#;
        assert_eq!(find_media(p).as_deref(), Some("https://s2.kino.example/v/master.m3u8?k=1&x=2"));
        assert_eq!(find_media("<p>нет видео</p>"), None);
        let base = Url::parse("https://kino.example/film/1").unwrap();
        let f = find_iframe(r#"<iframe width="1" src="/player/77"></iframe>"#, &base).unwrap();
        assert_eq!(f.as_str(), "https://kino.example/player/77");
        assert_eq!(find_iframe("<iframe src=/player/88 allowfullscreen>", &base).unwrap().path(), "/player/88");
        assert!(find_iframes("<iframe src=/embed/trailer/88>", &base).is_empty());
        let extra = vec!["kino.example".to_string()];
        assert!(allowed_for("site", &Url::parse("https://s2.kino.example/x.ts").unwrap(), &extra));
        assert!(!allowed_for("site", &Url::parse("https://evil.example/x.ts").unwrap(), &extra));
        assert!(!allowed_for("vk", &Url::parse("https://s2.kino.example/x.ts").unwrap(), &extra));
        assert!(allowed_for("direct", &Url::parse("https://s2.kino.example/x.ts").unwrap(), &extra));
        assert!(!allowed_for("direct", &Url::parse("https://evil.example/x.ts").unwrap(), &extra));
    }

    #[tokio::test]
    #[ignore = "requires the live external source and CDN"]
    async fn lordfilm_live_movie_playlist_and_segment() {
        tokio::time::timeout(Duration::from_secs(55), async {
            let found = crate::lordfilm::search("Дюна", Some(2021)).await.expect("source search");
            let url = found.first().expect("matching movie")["url"].as_str().unwrap();
            assert_eq!(super::super::links::parse(url).unwrap().kind, "site");
            let (stream, extra) = master("site", "", url, None).await.expect("native stream");
            let (base, response) = fetch_media("site", Url::parse(&stream).unwrap(), &extra, None).await.unwrap();
            assert!(response.status().is_success());
            let text = response.text().await.unwrap();
            assert!(text.starts_with("#EXTM3U"));
            let variant = text.lines().find(|s| !s.trim().is_empty() && !s.starts_with('#')).unwrap();
            let (base, response) = fetch_media("site", base.join(variant).unwrap(), &extra, None).await.unwrap();
            assert!(response.status().is_success());
            let text = response.text().await.unwrap();
            assert!(text.starts_with("#EXTM3U"));
            let segment = text.lines().find(|s| !s.trim().is_empty() && !s.starts_with('#')).unwrap();
            let range = axum::http::HeaderValue::from_static("bytes=0-1023");
            let (_, mut response) =
                fetch_media("site", base.join(segment).unwrap(), &extra, Some(&range)).await.unwrap();
            assert!(response.status().is_success());
            assert!(!response.chunk().await.unwrap().unwrap().is_empty());
        })
        .await
        .expect("live source timeout");
    }
}
