use crate::{
    auth::Viewer,
    discover,
    error::{ApiError, ApiResult},
    util::now_ms,
    App,
};
use axum::{
    extract::{Path, State},
    Json,
};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn trailer(State(app): State<Arc<App>>, _v: Viewer, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    if !discover::valid_id(&id) {
        return Err(ApiError::not_found());
    }
    let key = format!("trailer.ru.v4.{id}");
    if let Some(raw) = app.db.setting(&key) {
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            let ttl = if v["linkId"].is_null() { 600_000 } else { 86_400_000 };
            if now_ms() - v["at"].as_i64().unwrap_or(0) < ttl {
                return Ok(Json(v));
            }
        }
    }
    type Work = (String, Option<i64>);
    let work:Option<Work>=app.db.with(|c|c.query_row("SELECT COALESCE((SELECT title FROM title_localizations l WHERE l.id=e.id),title),year FROM ext_titles e WHERE id=?",[&id],|r|Ok((r.get(0)?,r.get(1)?))).optional())?;
    let work = if let Some(w) = work {
        w
    } else {
        let row: Option<Work> = app.db.with(|c| {
            c.query_row("SELECT title,year FROM titles WHERE imdb_id=? LIMIT 1", [&id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()
        })?;
        let (title, year) = row.ok_or_else(ApiError::not_found)?;
        discover::upsert(
            &app,
            &[discover::Ext {
                id: id.clone(),
                series: false,
                title: title.clone(),
                original: None,
                year: year.map(|y| y as i32),
                rating: None,
                votes: None,
                genres: vec![],
                poster_url: None,
            }],
        )?;
        (title, year)
    };
    let query = super::links::SearchQuery {
        provider: None,
        q: format!("{} {} русский дублированный трейлер", work.0, work.1.map(|v| v.to_string()).unwrap_or_default()),
        min: Some(20),
        title_id: None,
        ext_id: Some(id.clone()),
        season: None,
        episode: None,
        mode: Some("trailer".into()),
    };
    let curated: Value = serde_json::from_str(include_str!("../trailers-ru.json")).expect("bundled trailer catalogue");
    let mut found = if curated[&id].is_object() {
        vec![curated[&id].clone()]
    } else {
        crate::video_search::find(&app, &query).await.unwrap_or_default()
    };
    found.retain(|v| russian_audio(v["label"].as_str().unwrap_or_default()));
    found.sort_by_key(|v| std::cmp::Reverse(super::links::quality_hint(v["label"].as_str().unwrap_or_default())));
    let mut links = Vec::new();
    for video in found.iter().take(4) {
        if let Some(parsed) = video["url"].as_str().and_then(super::links::parse) {
            links.push(app.db.with(|c| {
                if let Some(id) = c
                    .query_row(
                        "SELECT id FROM links WHERE url=? AND title_id IS NULL AND ext_id IS NULL LIMIT 1",
                        [&parsed.url],
                        |r| r.get::<_, i64>(0),
                    )
                    .optional()?
                {
                    return Ok(id);
                }
                c.execute(
                    "INSERT INTO links(kind,url,embed,label,duration,added_at) VALUES(?,?,?,?,?,?)",
                    params![
                        parsed.kind,
                        parsed.url,
                        parsed.embed,
                        video["label"].as_str().unwrap_or("Трейлер"),
                        video["duration"].as_i64(),
                        now_ms()
                    ],
                )?;
                Ok(c.last_insert_rowid())
            })?);
        }
    }
    let result = json!({"linkId":links.first(),"linkIds":links,"at":now_ms()});
    app.db.set_setting(&key, &result.to_string())?;
    Ok(Json(result))
}

fn russian_audio(label: &str) -> bool {
    let label = label.to_lowercase().replace('ё', "е");
    if ["субтитр", "subtit", "english", "оригинал", "на английском"].iter().any(|s| label.contains(s))
    {
        return false;
    }
    ["русский", "на русском", "дублирован", "дубляж", "русская озвучка", "русскоязыч"].iter().any(|s| label.contains(s))
}

const HD: &str =
    r#"query T(: ID!) { title(id: ) { latestTrailer { playbackURLs { url videoMimeType videoDefinition } } } }"#;

/// Официальный трейлер IMDb в HD (mp4 1080p, иначе 720p) - для беззвучного баннера: у русских трейлеров
/// с сайтов картинка часто пережатая («720p» по подписи, мыло на деле). Ссылка IMDb подписана и живёт
/// сутки - держим 6 часов; нет HD - None (баннер возьмёт русский трейлер или останется на кадре).
async fn hd_url(app: &App, id: &str) -> Option<String> {
    let key = format!("trailer.hd.{id}");
    if let Some(v) = app.db.setting(&key).and_then(|raw| serde_json::from_str::<Value>(&raw).ok()) {
        if now_ms() - v["at"].as_i64().unwrap_or(0) < 6 * 3_600_000 {
            return v["url"].as_str().map(String::from);
        }
    }
    let v = crate::meta::imdb::Imdb::new(app.http.clone()).data(HD, json!({ "id": id }), "title").await.ok()?;
    let urls = v["latestTrailer"]["playbackURLs"].as_array()?.clone();
    let pick = |def: &str| {
        urls.iter()
            .find(|u| u["videoMimeType"] == "MP4" && u["videoDefinition"] == def)
            .and_then(|u| u["url"].as_str())
            .map(String::from)
    };
    let url = pick("DEF_1080p").or_else(|| pick("DEF_720p"));
    let _ = app.db.set_setting(&key, &json!({ "url": url, "at": now_ms() }).to_string());
    url
}

/// Видео официального трейлера через Nova (браузер на чужие сайты не ходит), с перемоткой (Range).
pub async fn trailer_hd(
    State(app): State<Arc<App>>,
    _v: Viewer,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
) -> ApiResult<axum::response::Response> {
    use axum::http::{header, StatusCode};
    if !discover::valid_id(&id) {
        return Err(ApiError::not_found());
    }
    let url = hd_url(&app, &id).await.ok_or_else(ApiError::not_found)?;
    let u = reqwest::Url::parse(&url).map_err(|_| ApiError::not_found())?;
    if !u.host_str().is_some_and(|h| h.ends_with(".media-imdb.com")) || !crate::outbound::valid_url(&u) {
        return Err(ApiError::not_found());
    }
    let mut req = app.http.get(u).timeout(std::time::Duration::from_secs(30));
    if let Some(r) = headers.get(header::RANGE) {
        req = req.header(header::RANGE, r.clone());
    }
    let r = req.send().await.map_err(|e| ApiError::bad("trailer", format!("IMDb не ответил: {e}")))?;
    if !r.status().is_success() {
        // Ссылка истекла раньше срока - в следующий раз взять новую.
        app.db.set_setting(&format!("trailer.hd.{id}"), "{}")?;
        return Err(ApiError::not_found());
    }
    let mut resp = axum::response::Response::builder()
        .status(StatusCode::from_u16(r.status().as_u16()).unwrap_or(StatusCode::OK))
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=3600");
    for h in [header::CONTENT_LENGTH, header::CONTENT_RANGE] {
        if let Some(v) = r.headers().get(&h) {
            resp = resp.header(h, v.clone());
        }
    }
    resp.body(axum::body::Body::from_stream(r.bytes_stream())).map_err(ApiError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn russian_title_alone_does_not_prove_russian_audio() {
        assert!(!russian_audio("Крестный отец - официальный трейлер 1080p"));
        assert!(!russian_audio("Крестный отец - русский трейлер с субтитрами"));
        assert!(!russian_audio("The Godfather official English trailer"));
        assert!(russian_audio("Крестный отец - русский дублированный трейлер 1080p"));
    }
}
