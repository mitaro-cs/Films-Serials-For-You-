//! Видео с других сайтов: вставить ссылку (Rutube, VK Видео, YouTube - их встроенным плеером
//! внутри нашего, прямые mp4/webm/m3u8 - нашим плеером) или найти на Rutube (VK - если хост
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
    /// Id у сайта (Rutube - для описания).
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
    if crate::lordfilm::movie_url(&u) || crate::lordfilm::episode_target(&u).is_some() {
        return Some(Parsed { kind: "site", url: u.to_string(), embed: None, id: None });
    }
    if u.scheme() != "https" && u.scheme() != "http" {
        return None;
    }
    let host = u.host_str()?.trim_start_matches("www.").trim_start_matches("m.").to_lowercase();
    let segs: Vec<&str> = u.path_segments().map(|s| s.filter(|p| !p.is_empty()).collect()).unwrap_or_default();
    let q = |k: &str| u.query_pairs().find(|(n, _)| n == k).map(|(_, v)| v.into_owned());
    match host.as_str() {
        "dailymotion.com" | "dai.ly" => {
            let id = if host == "dai.ly" { segs.first().copied()? } else { *segs.get(1)? };
            if !id.bytes().all(|b| b.is_ascii_alphanumeric()) || id.len() > 32 {
                return None;
            }
            Some(Parsed {
                kind: "site",
                url: format!("https://www.dailymotion.com/video/{id}"),
                embed: None,
                id: Some(id.into()),
            })
        }
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

/// Свои сайты хоста (`video.sites`): домены, с чьих страниц видео открывается в нашем плеере.
pub fn extra_sites(app: &App) -> Vec<String> {
    super::json_list(app.db.setting("video.sites"))
}

/// Домен сайта из того, что ввёл хост: «https://www.Kino.example/films» → «kino.example».
pub fn site_domain(raw: &str) -> Option<String> {
    let s = raw.trim().to_lowercase();
    let s = s.split("://").last().unwrap_or("");
    let host = s.split(['/', '?', '#']).next().unwrap_or("").trim_start_matches("www.");
    let ok = host.contains('.')
        && host.len() <= 100
        && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && !host.starts_with('.')
        && !host.ends_with('.');
    ok.then(|| host.to_string())
}

/// Как `parse`, а ссылка со своего сайта хоста - вид `site` (видео ищется на странице).
pub fn parse_with(raw: &str, sites: &[String]) -> Option<Parsed> {
    if let Some(p) = parse(raw) {
        return Some(p);
    }
    let u = Url::parse(raw.trim()).ok()?;
    if u.scheme() != "https" && u.scheme() != "http" {
        return None;
    }
    let host = u.host_str()?.trim_start_matches("www.").to_lowercase();
    sites.iter().any(|s| host == *s || host.ends_with(&format!(".{s}"))).then(|| Parsed {
        kind: "site",
        url: u.to_string(),
        embed: None,
        id: None,
    })
}

/// Название и картинка страницы своего сайта (`<title>`, `og:image`) - если ответит за 6 с.
async fn page_info(url: &str) -> (Option<String>, Option<String>) {
    let Ok(Ok(page)) = tokio::time::timeout(Duration::from_secs(6), super::stream_link::page(url)).await else {
        return (None, None);
    };
    let meta = |name: &str| {
        let i = page.find(&format!("property=\"{name}\""))?;
        let rest = &page[i..];
        let c = rest.find("content=\"")? + 9;
        let e = rest[c..].find('"')?;
        Some(rest[c..c + e].to_string()).filter(|s| !s.is_empty())
    };
    let title = meta("og:title").or_else(|| {
        let s = page.find("<title")?;
        let s = s + page[s..].find('>')? + 1;
        let e = page[s..].find("</title>")?;
        Some(page[s..s + e].trim().to_string()).filter(|t| !t.is_empty())
    });
    (title.map(|t| t.replace("&quot;", "\"").replace("&amp;", "&").replace("&#39;", "'")), meta("og:image"))
}

const COLS: &str = "l.id, l.kind, l.url, l.embed, l.label, NULLIF(l.thumb, ''), l.duration, l.title_id, l.ext_id, \
    l.added_by, COALESCE(p.name, ''), l.added_at, lp.position, lp.duration, l.season, l.episode";

/// Ссылки с автором и местом, где остановился этот зритель (`?1` - его профиль).
const FROM: &str = "links l LEFT JOIN profiles p ON p.id = l.added_by
    LEFT JOIN link_progress lp ON lp.link_id = l.id AND lp.profile_id = ?1";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?, "kind": r.get::<_, String>(1)?, "url": r.get::<_, String>(2)?,
        "embed": r.get::<_, Option<String>>(3)?, "label": r.get::<_, String>(4)?,
        "thumb": super::img(r.get(5)?), "duration": r.get::<_, Option<i64>>(6)?,
        "titleId": r.get::<_, Option<i64>>(7)?, "extId": r.get::<_, Option<String>>(8)?,
        "addedBy": r.get::<_, Option<i64>>(9)?, "author": r.get::<_, String>(10)?, "addedAt": r.get::<_, i64>(11)?,
        "progress": r.get::<_, Option<f64>>(12)?.map(|p| json!({"position": p, "duration": r.get::<_, Option<f64>>(13).ok().flatten().unwrap_or(0.0)})),
        "season": r.get::<_, Option<i64>>(14)?, "episode": r.get::<_, Option<i64>>(15)?,
    }))
}

/// Ссылки фильма с сервиса, фильма из ленты или «просто ссылки» (оба None).
pub fn of(app: &App, profile: i64, title: Option<i64>, ext: Option<&str>) -> ApiResult<Vec<Value>> {
    let sql = format!(
        "SELECT {COLS} FROM {FROM}
         WHERE l.title_id IS ?2 AND l.ext_id IS ?3 ORDER BY lp.updated_at IS NULL, lp.updated_at DESC, l.added_at DESC LIMIT 200"
    );
    Ok(app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let r = st.query_map(params![profile, title, ext], row)?;
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
    Ok(Json(json!(of(&app, v.profile.id, q.title_id, q.ext_id.as_deref())?)))
}

pub async fn one(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let sql = format!("SELECT {COLS} FROM {FROM} WHERE l.id = ?2");
    Ok(Json(
        app.db
            .with(|c| c.query_row(&sql, params![v.profile.id, id], row).optional())?
            .ok_or_else(ApiError::not_found)?,
    ))
}

#[derive(Deserialize)]
pub struct ProgressBody {
    position: f64,
    duration: f64,
}

/// Где остановился у видео по ссылке. Фильм из ленты сам становится «Смотрю», досмотрен (95 %) -
/// «Просмотрел».
pub async fn progress(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<ProgressBody>,
) -> ApiResult<Json<Value>> {
    if !(b.position.is_finite() && b.duration.is_finite() && b.position >= 0.0 && b.duration > 0.0) {
        return Err(ApiError::bad("bad_progress", "Неверное место в видео"));
    }
    let target: Option<(Option<String>, Option<i64>, Option<i64>)> = app.db.with(|c| {
        c.query_row("SELECT ext_id, season, episode FROM links WHERE id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()
    })?;
    let (ext, season, episode) = target.ok_or_else(ApiError::not_found)?;
    let (me, now) = (v.profile.id, now_ms());
    let done = b.position >= b.duration * 0.95;
    app.db.tx(|c| {
        c.execute(
            "INSERT INTO link_progress (profile_id, link_id, position, duration, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(profile_id, link_id) DO UPDATE SET position = ?3, duration = ?4, updated_at = ?5",
            params![me, id, b.position, b.duration, now],
        )?;
        if let Some(e) = &ext {
            c.execute("DELETE FROM ext_dropped WHERE profile_id = ? AND ext_id = ?", params![me, e])?;
            let series: bool = c.query_row("SELECT kind = 'series' FROM ext_titles WHERE id = ?", [e], |r| r.get(0))?;
            if done {
                if let (Some(s), Some(ep)) = (season, episode) {
                    c.execute("INSERT OR IGNORE INTO ext_seen (profile_id, ext_id, season, episode, at) VALUES (?, ?, ?, ?, ?)", params![me, e, s, ep, now])?;
                }
            }
            let status = if done && !series { "watched" } else { "watching" };
            c.execute(
                "INSERT INTO ext_marks (profile_id, ext_id, status, at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(profile_id, ext_id) DO UPDATE SET status = ?3, at = ?4
                 WHERE ext_marks.status != 'watched' OR ?3 = 'watched'",
                params![me, e, status, now],
            )?;
        }
        Ok(())
    })?;
    Ok(Json(json!({"ok": true})))
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
    season: Option<i64>,
    episode: Option<i64>,
}

const BAD_LINK: &str =
    "Такую ссылку не открыть: нужна ссылка на Rutube, VK Видео, YouTube, прямая на .mp4, .webm, .m3u8 \
     или на сайт, который хост добавил в «Управление → Настройки → Свои сайты»";

/// Описание видео на Rutube (название, картинка, длительность) - если сайт ответит за 5 с.
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

/// Картинку видео - к себе (браузер ходит только к нам). Только https и только с известных сайтов.
async fn keep_thumb(app: &App, url: Option<&str>) -> Option<String> {
    let u = Url::parse(url?).ok()?;
    if u.scheme() != "https" || !crate::outbound::valid_url(&u) {
        return None;
    }
    tokio::time::timeout(Duration::from_secs(6), app.images.download(u.as_str())).await.ok()?.ok()
}

#[derive(Deserialize)]
pub struct ThumbnailQuery {
    url: String,
}

pub async fn thumbnail(
    State(app): State<Arc<App>>,
    _v: Viewer,
    Query(q): Query<ThumbnailQuery>,
) -> ApiResult<axum::response::Redirect> {
    if q.url.len() > 4096 {
        return Err(ApiError::not_found());
    }
    let name = keep_thumb(&app, Some(&q.url)).await.ok_or_else(ApiError::not_found)?;
    Ok(axum::response::Redirect::temporary(&format!("/img/{name}")))
}

/// Картинки ссылок Rutube, сохранённых без них (до 1.0 картинки с rtbcdn.ru не скачивались), -
/// один раз после запуска.
pub async fn backfill_thumbs(app: Arc<App>) {
    tokio::time::sleep(Duration::from_secs(20)).await;
    let todo: Vec<(i64, String)> = app
        .db
        .with(|c| {
            let mut st = c.prepare(
                "SELECT id, url FROM links WHERE kind = 'rutube' AND (thumb IS NULL OR thumb = '') ORDER BY id DESC LIMIT 200",
            )?;
            let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            r.collect()
        })
        .unwrap_or_default();
    for (id, url) in todo {
        let Some(rid) = parse(&url).and_then(|p| p.id) else { continue };
        let Some((_, th, _)) = rutube_info(&app, &rid).await else { continue };
        if let Some(name) = keep_thumb(&app, th.as_deref()).await {
            let _ = app.db.with(|c| c.execute("UPDATE links SET thumb = ? WHERE id = ?", params![name, id]));
        }
    }
}

pub async fn add(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<NewLink>) -> ApiResult<Json<Value>> {
    if b.season.is_some() != b.episode.is_some()
        || b.season.is_some_and(|s| !(0..=1000).contains(&s))
        || b.episode.is_some_and(|e| !(1..=10000).contains(&e))
    {
        return Err(ApiError::bad("bad_episode", "Укажите сезон и номер серии"));
    }
    let p = parse_with(&b.url, &extra_sites(&app)).ok_or_else(|| ApiError::bad("bad_link", BAD_LINK))?;
    if let Some((season, episode)) = Url::parse(&p.url).ok().as_ref().and_then(crate::lordfilm::episode_target) {
        if b.season != Some(season) || b.episode != Some(episode) {
            return Err(ApiError::bad("bad_episode", "Серия в ссылке не совпадает с выбранной серией"));
        }
    }
    if b.title_id.is_some() && b.ext_id.is_some() {
        return Err(ApiError::bad("bad_link", "Ссылка - либо к фильму с сервиса, либо к фильму из ленты"));
    }
    if let Some(t) = b.title_id {
        super::cards_by_ids(&app, v.profile.kids, &[t])?.pop().ok_or_else(ApiError::not_found)?;
    }
    if let Some(e) = &b.ext_id {
        super::discover::ext_card(&app, &v, e)?;
    }
    let existing: Option<i64> = app.db.with(|c| c.query_row(
        "SELECT id FROM links WHERE url = ?1 AND title_id IS ?2 AND ext_id IS ?3 AND season IS ?4 AND episode IS ?5 LIMIT 1",
        params![p.url, b.title_id, b.ext_id, b.season, b.episode], |r| r.get(0),
    ).optional())?;
    if let Some(id) = existing {
        return Ok(Json(json!({"id": id})));
    }
    let n: i64 =
        app.db.with(|c| c.query_row("SELECT COUNT(*) FROM links WHERE added_by = ?", [v.profile.id], |r| r.get(0)))?;
    if n >= 500 {
        return Err(ApiError::bad("too_many", "Не больше 500 своих ссылок"));
    }
    let label = b
        .label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Видео")
        .chars()
        .take(200)
        .collect::<String>();
    let thumb: Option<String> = None;
    let id = app.db.with(|c| {
        c.execute(
            "INSERT INTO links (title_id, ext_id, kind, url, embed, label, thumb, duration, added_by, added_at, season, episode)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![b.title_id, b.ext_id, p.kind, p.url, p.embed, label, thumb, b.duration, v.profile.id, now_ms(), b.season, b.episode],
        )?;
        Ok(c.last_insert_rowid())
    })?;
    // Starting playback must never wait for a thumbnail or remote page metadata.
    let background = app.clone();
    tokio::spawn(async move {
        let mut image = b.thumb;
        let mut title = None;
        let mut duration = b.duration;
        if p.kind == "rutube" && (image.is_none() || b.label.is_none()) {
            if let Some((name, thumb, seconds)) = rutube_info(&background, p.id.as_deref().unwrap_or_default()).await {
                if b.label.is_none() {
                    title = Some(name);
                }
                image = image.or(thumb);
                duration = duration.or(seconds);
            }
        } else if p.kind == "site" && (image.is_none() || b.label.is_none()) {
            let (name, thumb) = page_info(&p.url).await;
            if b.label.is_none() {
                title = name;
            }
            image = image.or(thumb);
        }
        let thumb = keep_thumb(&background, image.as_deref()).await;
        let _ = background.db.with(|c|c.execute("UPDATE links SET thumb=COALESCE(?1,thumb),label=COALESCE(?2,label),duration=COALESCE(duration,?3) WHERE id=?4",params![thumb,title,duration,id]));
    });
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
    pub q: String,
    pub provider: Option<String>,
    #[serde(rename = "titleId")]
    pub title_id: Option<i64>,
    #[serde(rename = "extId")]
    pub ext_id: Option<String>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub mode: Option<String>,
    /// Только видео не короче стольких секунд (целый фильм, а не трейлер) - до скачивания картинок.
    pub min: Option<i64>,
}

/// Подтверждённая метаданными или заявленная в названии высота; неизвестную не выдумываем.
pub fn quality_hint(label: &str) -> u32 {
    let label = label.to_lowercase();
    for (height, tags) in [
        (2160, &["2160", "4k", "4к", "uhd"][..]),
        (1440, &["1440", "2k"][..]),
        (1080, &["1080", "full hd", "fullhd"][..]),
        (720, &["720"][..]),
        (480, &["480"][..]),
    ] {
        if tags.iter().any(|tag| label.contains(tag)) {
            return height;
        }
    }
    0
}

/// Поиск видео: Rutube всегда, VK Видео - если хост указал ключ VK. Картинки - к себе.
pub async fn search(State(app): State<Arc<App>>, v: Viewer, Query(q): Query<SearchQuery>) -> ApiResult<Json<Value>> {
    if let Some(id) = q.title_id {
        super::cards_by_ids(&app, v.profile.kids, &[id])?.pop().ok_or_else(ApiError::not_found)?;
    }
    if let Some(id) = &q.ext_id {
        super::discover::ext_card(&app, &v, id)?;
    }
    let mut items = crate::video_search::find(&app, &q).await.map_err(|e| ApiError::bad("search", e.to_string()))?;
    // Отмеченные «не тот фильм» - не предлагаем.
    let bad: std::collections::HashSet<String> = app.db.with(|c| {
        let mut st = c.prepare("SELECT url FROM bad_videos")?;
        let r = st.query_map([], |r| r.get(0))?;
        r.collect::<rusqlite::Result<_>>()
    })?;
    items.retain(|i| i["url"].as_str().is_none_or(|u| !bad.contains(u)));
    Ok(Json(json!(items)))
}

/// «Это не тот фильм»: видео по ссылке оказалось другим (ролик с похожим названием, трейлер).
/// Адрес - в чёрный список поиска для всех, ссылка у фильма удаляется (решать может любой зритель:
/// неверное видео неверно для всех).
pub async fn wrong(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    type Row = (String, Option<i64>, Option<String>);
    let row: Option<Row> = app.db.with(|c| {
        c.query_row("SELECT url, title_id, ext_id FROM links WHERE id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()
    })?;
    let (url, title, ext) = row.ok_or_else(ApiError::not_found)?;
    app.db.tx(|tx| {
        tx.execute(
            "INSERT OR REPLACE INTO bad_videos (url, title_id, ext_id, by_profile, at) VALUES (?, ?, ?, ?, ?)",
            params![url, title, ext, v.profile.id, now_ms()],
        )?;
        tx.execute("DELETE FROM links WHERE url = ? AND (title_id IS ?2 AND ext_id IS ?3)", params![url, title, ext])?;
        Ok(())
    })?;
    Ok(Json(json!({"ok": true})))
}

/// Где зритель остановился в этой работе (та же серия) по любому её видео: на другом устройстве
/// могли открыть другой источник того же фильма.
pub async fn resume(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let row: Option<(i64, f64, f64, i64)> = app.db.with(|c| {
        c.query_row(
            "SELECT o.id, p.position, p.duration, p.updated_at
               FROM links l
               JOIN links o ON (o.id = l.id
                   OR (l.ext_id IS NOT NULL AND o.ext_id = l.ext_id)
                   OR (l.title_id IS NOT NULL AND o.title_id = l.title_id))
                 AND o.season IS l.season AND o.episode IS l.episode
               JOIN link_progress p ON p.link_id = o.id AND p.profile_id = ?1
              WHERE l.id = ?2
              ORDER BY p.updated_at DESC LIMIT 1",
            params![v.profile.id, id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
    })?;
    Ok(Json(match row {
        Some((link, position, duration, at)) => {
            json!({"linkId": link, "position": position, "duration": duration, "updatedAt": at})
        }
        None => json!({"linkId": null, "position": null, "duration": null, "updatedAt": null}),
    }))
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
