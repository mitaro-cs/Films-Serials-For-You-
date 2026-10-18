//! Видео с других сайтов: вставить ссылку (Rutube, VK Видео, YouTube — их встроенным плеером
//! внутри нашего, прямые mp4/webm/m3u8 — нашим плеером) или найти на Rutube (VK — если хост
//! указал ключ VK). Ссылка привязывается к фильму с сервиса, к фильму из ленты или живёт сама.

use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::util::now_ms;
use crate::App;
use axum::extract::{Path, Query, State};
use axum::Json;
use reqwest::Url;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

/// Разобранная ссылка: вид и адрес встроенного плеера.
#[derive(Debug, PartialEq)]
pub struct Parsed {
    pub kind: &'static str,
    pub url: String,
    pub embed: Option<String>,
    /// Id у сайта (Rutube — для описания).
    pub id: Option<String>,
}

fn hex32(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `-123_456` → ("-123", "456").
fn vk_pair(s: &str) -> Option<(String, String)> {
    let (o, i) = s.split_once('_')?;
    let ok = |x: &str, neg: bool| {
        let d = if neg { x.strip_prefix('-').unwrap_or(x) } else { x };
        !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())
    };
    (ok(o, true) && ok(i, false)).then(|| (o.to_string(), i.to_string()))
}

pub fn parse(raw: &str) -> Option<Parsed> {
    let u = Url::parse(raw.trim()).ok()?;
    if u.scheme() != "https" && u.scheme() != "http" {
        return None;
    }
    let host = u.host_str()?.trim_start_matches("www.").trim_start_matches("m.").to_lowercase();
    let segs: Vec<&str> = u.path_segments().map(|s| s.filter(|p| !p.is_empty()).collect()).unwrap_or_default();
    let q = |k: &str| u.query_pairs().find(|(n, _)| n == k).map(|(_, v)| v.into_owned());
    match host.as_str() {
        "rutube.ru" => {
            let id = match segs.as_slice() {
                ["video", id, ..] | ["play", "embed", id, ..] | ["shorts", id, ..] => *id,
                _ => return None,
            };
            hex32(id).then(|| Parsed {
                kind: "rutube",
                url: format!("https://rutube.ru/video/{id}/"),
                embed: Some(format!("https://rutube.ru/play/embed/{id}")),
                id: Some(id.to_string()),
            })
        }
        "vk.com" | "vkvideo.ru" | "vk.ru" => {
            let (oid, id, hash) = if segs.first() == Some(&"video_ext.php") {
                (q("oid")?, q("id")?, q("hash"))
            } else {
                let key = q("z")
                    .and_then(|z| z.strip_prefix("video").map(|v| v.split('/').next().unwrap_or("").to_string()))
                    .or_else(|| segs.last().and_then(|s| s.strip_prefix("video")).map(String::from))?;
                let (o, i) = vk_pair(&key)?;
                (o, i, None)
            };
            vk_pair(&format!("{oid}_{id}"))?;
            let mut embed = format!("https://vkvideo.ru/video_ext.php?oid={oid}&id={id}&hd=2");
            if let Some(h) = hash.filter(|h| h.bytes().all(|b| b.is_ascii_alphanumeric())) {
                embed.push_str(&format!("&hash={h}"));
            }
            Some(Parsed {
                kind: "vk",
                url: format!("https://vkvideo.ru/video{oid}_{id}"),
                embed: Some(embed),
                id: Some(format!("{oid}_{id}")),
            })
        }
        "youtube.com" | "youtu.be" | "youtube-nocookie.com" => {
            let id = if host == "youtu.be" {
                segs.first().map(|s| s.to_string())
            } else if matches!(segs.first(), Some(&"embed") | Some(&"shorts") | Some(&"live")) {
                segs.get(1).map(|s| s.to_string())
            } else {
                q("v")
            }?;
            (id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')).then(|| {
                Parsed {
                    kind: "youtube",
                    url: format!("https://www.youtube.com/watch?v={id}"),
                    embed: Some(format!("https://www.youtube-nocookie.com/embed/{id}?autoplay=1")),
                    id: Some(id),
                }
            })
        }
        _ => {
            let path = u.path().to_lowercase();
            [".mp4", ".m4v", ".webm", ".m3u8", ".mov"].iter().any(|e| path.ends_with(e)).then(|| Parsed {
                kind: "direct",
                url: u.to_string(),
                embed: None,
                id: None,
            })
        }
    }
}

const COLS: &str = "l.id, l.kind, l.url, l.embed, l.label, NULLIF(l.thumb, ''), l.duration, l.title_id, l.ext_id, \
    l.added_by, COALESCE(p.name, ''), l.added_at";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?, "kind": r.get::<_, String>(1)?, "url": r.get::<_, String>(2)?,
        "embed": r.get::<_, Option<String>>(3)?, "label": r.get::<_, String>(4)?,
        "thumb": super::img(r.get(5)?), "duration": r.get::<_, Option<i64>>(6)?,
        "titleId": r.get::<_, Option<i64>>(7)?, "extId": r.get::<_, Option<String>>(8)?,
        "addedBy": r.get::<_, Option<i64>>(9)?, "author": r.get::<_, String>(10)?, "addedAt": r.get::<_, i64>(11)?,
    }))
}

/// Ссылки фильма с сервиса, фильма из ленты или «просто ссылки» (оба None).
pub fn of(app: &App, title: Option<i64>, ext: Option<&str>) -> ApiResult<Vec<Value>> {
    let sql = format!(
        "SELECT {COLS} FROM links l LEFT JOIN profiles p ON p.id = l.added_by
         WHERE l.title_id IS ?1 AND l.ext_id IS ?2 ORDER BY l.added_at DESC LIMIT 200"
    );
    Ok(app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let r = st.query_map(params![title, ext], row)?;
        r.collect::<rusqlite::Result<Vec<_>>>()
    })?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    title_id: Option<i64>,
    ext_id: Option<String>,
}

pub async fn list(State(app): State<Arc<App>>, v: Viewer, Query(q): Query<ListQuery>) -> ApiResult<Json<Value>> {
    if let Some(t) = q.title_id {
        super::cards_by_ids(&app, v.profile.kids, &[t])?.pop().ok_or_else(ApiError::not_found)?;
    }
    Ok(Json(json!(of(&app, q.title_id, q.ext_id.as_deref())?)))
}

pub async fn one(State(app): State<Arc<App>>, _v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let sql = format!("SELECT {COLS} FROM links l LEFT JOIN profiles p ON p.id = l.added_by WHERE l.id = ?");
    Ok(Json(app.db.with(|c| c.query_row(&sql, [id], row).optional())?.ok_or_else(ApiError::not_found)?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewLink {
    url: String,
    title_id: Option<i64>,
    ext_id: Option<String>,
    label: Option<String>,
    thumb: Option<String>,
    duration: Option<i64>,
}

const BAD_LINK: &str =
    "Такую ссылку не открыть: нужна ссылка на Rutube, VK Видео, YouTube или прямая на .mp4, .webm, .m3u8";

/// Описание видео на Rutube (название, картинка, длительность) — если сайт ответит за 5 с.
async fn rutube_info(app: &App, id: &str) -> Option<(String, Option<String>, Option<i64>)> {
    let v: Value = app
        .http
        .get(format!("https://rutube.ru/api/video/{id}/"))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    Some((v["title"].as_str()?.to_string(), v["thumbnail_url"].as_str().map(String::from), v["duration"].as_i64()))
}

/// Картинку видео — к себе (браузер ходит только к нам). Только https и только с известных сайтов.
async fn keep_thumb(app: &App, url: Option<&str>) -> Option<String> {
    let u = Url::parse(url?).ok()?;
    let host = u.host_str()?.to_lowercase();
    let ok = ["rutube.ru", "userapi.com", "vk.com", "vkvideo.ru", "ytimg.com", "mycdn.me"]
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{h}")));
    if u.scheme() != "https" || !ok {
        return None;
    }
    tokio::time::timeout(Duration::from_secs(6), app.images.download(u.as_str())).await.ok()?.ok()
}

pub async fn add(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<NewLink>) -> ApiResult<Json<Value>> {
    let p = parse(&b.url).ok_or_else(|| ApiError::bad("bad_link", BAD_LINK))?;
    if b.title_id.is_some() && b.ext_id.is_some() {
        return Err(ApiError::bad("bad_link", "Ссылка — либо к фильму с сервиса, либо к фильму из ленты"));
    }
    if let Some(t) = b.title_id {
        super::cards_by_ids(&app, v.profile.kids, &[t])?.pop().ok_or_else(ApiError::not_found)?;
    }
    if let Some(e) = &b.ext_id {
        super::discover::ext_card(&app, &v, e)?;
    }
    let n: i64 =
        app.db.with(|c| c.query_row("SELECT COUNT(*) FROM links WHERE added_by = ?", [v.profile.id], |r| r.get(0)))?;
    if n >= 500 {
        return Err(ApiError::bad("too_many", "Не больше 500 своих ссылок"));
    }
    let mut label = b.label.as_deref().map(str::trim).filter(|l| !l.is_empty()).map(String::from);
    let mut thumb_url = b.thumb.clone();
    let mut duration = b.duration;
    if p.kind == "rutube" && (label.is_none() || thumb_url.is_none()) {
        if let Some((t, th, d)) = rutube_info(&app, p.id.as_deref().unwrap_or_default()).await {
            label = label.or(Some(t));
            thumb_url = thumb_url.or(th);
            duration = duration.or(d);
        }
    }
    let label: String = label
        .unwrap_or_else(|| match p.kind {
            "rutube" => "Видео на Rutube".into(),
            "vk" => "Видео ВКонтакте".into(),
            "youtube" => "Видео на YouTube".into(),
            _ => Url::parse(&p.url).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or_else(|| "Видео".into()),
        })
        .chars()
        .take(200)
        .collect();
    let thumb = keep_thumb(&app, thumb_url.as_deref()).await;
    let id = app.db.with(|c| {
        c.execute(
            "INSERT INTO links (title_id, ext_id, kind, url, embed, label, thumb, duration, added_by, added_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![b.title_id, b.ext_id, p.kind, p.url, p.embed, label, thumb, duration, v.profile.id, now_ms()],
        )?;
        Ok(c.last_insert_rowid())
    })?;
    Ok(Json(json!({"id": id})))
}

pub async fn delete(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let by: Option<Option<i64>> =
        app.db.with(|c| c.query_row("SELECT added_by FROM links WHERE id = ?", [id], |r| r.get(0)).optional())?;
    let by = by.ok_or_else(ApiError::not_found)?;
    if by != Some(v.profile.id) && !v.manages() {
        return Err(ApiError::new(axum::http::StatusCode::FORBIDDEN, "not_yours", "Удалить может тот, кто добавил"));
    }
    app.db.with(|c| c.execute("DELETE FROM links WHERE id = ?", [id]))?;
    Ok(Json(json!({"ok": true})))
}

/// Результаты Rutube (`/api/search/video/`): видео, без каналов и плейлистов.
pub fn parse_rutube(v: &Value) -> Vec<Value> {
    v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let id = r["id"].as_str().filter(|i| hex32(i))?;
            Some(json!({
                "site": "rutube", "url": format!("https://rutube.ru/video/{id}/"),
                "label": r["title"].as_str().unwrap_or("Видео"), "thumbUrl": r["thumbnail_url"].as_str(),
                "duration": r["duration"].as_i64(), "author": r["author"]["name"].as_str(),
            }))
        })
        .collect()
}

/// Результаты VK API `video.search`.
pub fn parse_vk(v: &Value) -> Vec<Value> {
    v["response"]["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let (o, i) = (r["owner_id"].as_i64()?, r["id"].as_i64()?);
            let thumb =
                r["image"].as_array().and_then(|a| a.iter().rev().nth(1).or(a.last())).and_then(|x| x["url"].as_str());
            Some(json!({
                "site": "vk", "url": format!("https://vkvideo.ru/video{o}_{i}"),
                "label": r["title"].as_str().unwrap_or("Видео"), "thumbUrl": thumb,
                "duration": r["duration"].as_i64(), "author": Value::Null,
            }))
        })
        .collect()
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: String,
    /// Только видео не короче стольких секунд (целый фильм, а не трейлер) — до скачивания картинок.
    min: Option<i64>,
}

/// Поиск видео: Rutube всегда, VK Видео — если хост указал ключ VK. Картинки — к себе.
pub async fn search(State(app): State<Arc<App>>, _v: Viewer, Query(q): Query<SearchQuery>) -> ApiResult<Json<Value>> {
    let query = q.q.trim();
    if query.chars().count() < 2 {
        return Ok(Json(json!([])));
    }
    let rutube = async {
        let u = Url::parse_with_params("https://rutube.ru/api/search/video/", &[("query", query)]).ok()?;
        let v: Value = app.http.get(u).timeout(Duration::from_secs(8)).send().await.ok()?.json().await.ok()?;
        Some(parse_rutube(&v))
    };
    let token = app.db.setting_or("vk.token", "");
    let vk = async {
        if token.is_empty() {
            return None;
        }
        let u = Url::parse_with_params(
            "https://api.vk.com/method/video.search",
            &[("q", query), ("count", "12"), ("adult", "0"), ("access_token", token.as_str()), ("v", "5.199")],
        )
        .ok()?;
        let v: Value = app.http.get(u).timeout(Duration::from_secs(8)).send().await.ok()?.json().await.ok()?;
        Some(parse_vk(&v))
    };
    let (r, k) = tokio::join!(rutube, vk);
    if r.is_none() && k.is_none() {
        return Err(ApiError::bad("search", "Rutube не ответил — попробуйте ещё раз или вставьте ссылку"));
    }
    let long = |x: &Value| q.min.is_none_or(|m| x["duration"].as_i64().unwrap_or(0) >= m);
    let mut out: Vec<Value> = r.unwrap_or_default().into_iter().filter(long).take(12).collect();
    out.extend(k.unwrap_or_default().into_iter().filter(long).take(12));
    let thumbs = out.iter().map(|x| keep_thumb(&app, x["thumbUrl"].as_str()));
    let thumbs = futures_util::future::join_all(thumbs).await;
    for (x, t) in out.iter_mut().zip(thumbs) {
        x["thumb"] = json!(super::img(t));
    }
    Ok(Json(json!(out)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_links() {
        let r = parse("https://rutube.ru/video/0123456789abcdef0123456789abcdef/?r=wd").unwrap();
        assert_eq!(r.kind, "rutube");
        assert_eq!(r.embed.as_deref(), Some("https://rutube.ru/play/embed/0123456789abcdef0123456789abcdef"));
        let v = parse("https://vk.com/video-123_456789").unwrap();
        assert_eq!(v.embed.as_deref(), Some("https://vkvideo.ru/video_ext.php?oid=-123&id=456789&hd=2"));
        let v = parse("https://vkvideo.ru/video?z=video-1_2%2Fpl_cat").unwrap();
        assert_eq!(v.id.as_deref(), Some("-1_2"));
        let v = parse("https://vk.com/video_ext.php?oid=5&id=7&hash=abc123").unwrap();
        assert!(v.embed.unwrap().ends_with("&hash=abc123"));
        let y = parse("https://youtu.be/dQw4w9WgXcQ").unwrap();
        assert_eq!(y.embed.as_deref(), Some("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?autoplay=1"));
        assert_eq!(parse("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1").unwrap().kind, "youtube");
        assert_eq!(parse("https://cdn.example.com/a/movie.MP4").unwrap().kind, "direct");
        assert!(parse("https://example.com/page").is_none());
        assert!(parse("javascript:alert(1)").is_none());
        assert!(parse("https://rutube.ru/video/short/").is_none());
        assert!(parse("https://vk.com/video-1_x").is_none());
    }

    #[test]
    fn parses_search() {
        let r = parse_rutube(&json!({"results": [
            {"id": "0123456789abcdef0123456789abcdef", "title": "Фильм", "thumbnail_url": "https://pic.rutube.ru/a.jpg",
             "duration": 6000, "author": {"name": "Канал"}},
            {"id": "bad", "title": "x"}
        ]}));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["url"], "https://rutube.ru/video/0123456789abcdef0123456789abcdef/");
        let k = parse_vk(&json!({"response": {"items": [{"owner_id": -1, "id": 2, "title": "В", "duration": 5,
            "image": [{"url": "https://sun.userapi.com/s.jpg"}, {"url": "https://sun.userapi.com/m.jpg"}]}]}}));
        assert_eq!(k[0]["url"], "https://vkvideo.ru/video-1_2");
        assert_eq!(k[0]["thumbUrl"], "https://sun.userapi.com/s.jpg");
    }
}
