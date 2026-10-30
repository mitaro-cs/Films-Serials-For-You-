//! Следующая серия: только точный сосед в том же сериале, без подстановки случайных роликов.
use crate::{
    auth::Viewer,
    error::{ApiError, ApiResult},
    App,
};
use axum::{
    extract::{Path, State},
    Json,
};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

pub async fn next(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    type Target = (Option<i64>, Option<String>, Option<i64>, Option<i64>, String);
    let target: Option<Target> = app.db.with(|c| {
        c.query_row("SELECT title_id,ext_id,season,episode,url FROM links WHERE id=?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .optional()
    })?;
    let (title, ext, Some(season), Some(episode), source_url) = target.ok_or_else(ApiError::not_found)? else {
        return Ok(Json(json!({"next":null})));
    };
    let lordfilm = reqwest::Url::parse(&source_url).ok().as_ref().is_some_and(crate::lordfilm::series_url);
    if let Some(t) = title {
        super::cards_by_ids(&app, v.profile.kids, &[t])?.pop().ok_or_else(ApiError::not_found)?;
    }
    if let Some(e) = &ext {
        super::discover::ext_card(&app, &v, e)?;
    }
    let existing = |s: i64, e: i64| -> ApiResult<Option<(i64, String)>> {
        Ok(app.db.with(|c|c.query_row("SELECT id,label FROM links WHERE title_id IS ? AND ext_id IS ? AND season=? AND episode=? AND kind!='youtube' AND (?=0 OR url LIKE 'https://sd.lordfilm5.pro/%') ORDER BY added_at DESC LIMIT 1",params![title,ext,s,e,lordfilm],|r|Ok((r.get(0)?,r.get(1)?))).optional())?)
    };
    if let Some((id, label)) = existing(season, episode + 1)? {
        return Ok(Json(json!({"next":{"id":id,"title":label}})));
    }
    let Some(ext) = ext.as_ref() else { return Ok(Json(json!({"next":null}))) };
    let known: bool = app.db.with(|c| {
        c.query_row(
            "SELECT EXISTS(SELECT 1 FROM ext_episodes WHERE ext_id=? AND season=?)",
            params![ext, season],
            |r| r.get(0),
        )
    })?;
    if !known {
        let _ =
            tokio::time::timeout(Duration::from_secs(5), crate::media_meta::episodes(&app, ext, season as i32)).await;
    }
    let (name,extra):(String,Option<String>)=app.db.with(|c|c.query_row("SELECT COALESCE((SELECT title FROM title_localizations WHERE id=e.id),title),extra FROM ext_titles e WHERE id=?",[ext],|r|Ok((r.get(0)?,r.get(1)?))))?;
    let max: Option<i64> = app.db.with(|c| {
        c.query_row("SELECT MAX(episode) FROM ext_episodes WHERE ext_id=? AND season=?", params![ext, season], |r| {
            r.get(0)
        })
    })?;
    let x: Value = extra.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    let (s, e) = if max.is_some_and(|max| episode < max) {
        (season, episode + 1)
    } else if max == Some(episode)
        && x["seasons"].as_array().is_some_and(|a| a.iter().any(|n| n.as_i64() == Some(season + 1)))
    {
        (season + 1, 1)
    } else {
        return Ok(Json(json!({"next":null})));
    };
    if let Some((id, label)) = existing(s, e)? {
        return Ok(Json(json!({"next":{"id":id,"title":label}})));
    }
    let query = super::links::SearchQuery {
        provider: lordfilm.then(|| "lordfilm".to_owned()),
        q: format!("{name} {s} сезон {e} серия"),
        min: Some(600),
        title_id: None,
        ext_id: Some(ext.clone()),
        season: Some(s),
        episode: Some(e),
        mode: None,
    };
    let found = crate::video_search::find(&app, &query).await.unwrap_or_default();
    let Some(video) = found.first() else {
        return Ok(Json(json!({"next":null,"unavailable":true,"season":s,"episode":e})));
    };
    let Some(parsed) = video["url"].as_str().and_then(super::links::parse) else {
        return Ok(Json(json!({"next":null})));
    };
    let label = format!("{name} · {s} сезон · {e} серия");
    let id=app.db.with(|c|{
        if let Some(id)=c.query_row("SELECT id FROM links WHERE url=? AND ext_id=? AND season=? AND episode=?",params![parsed.url,ext,s,e],|r|r.get::<_,i64>(0)).optional()?{return Ok(id)}
        c.execute("INSERT INTO links(kind,url,embed,label,duration,title_id,ext_id,season,episode,added_at) VALUES(?,?,?,?,?,?,?,?,?,?)",params![parsed.kind,parsed.url,parsed.embed,label,video["duration"].as_i64(),title,ext,s,e,crate::util::now_ms()])?;
        Ok(c.last_insert_rowid())
    })?;
    Ok(Json(json!({"next":{"id":id,"title":label}})))
}
