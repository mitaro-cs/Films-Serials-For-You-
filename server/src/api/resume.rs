use crate::{auth::Viewer, error::ApiResult, App};
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn list(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let taste = crate::taste::Taste::load(&app, v.profile.id)?;
    let mut cards = super::cards(
        &app,
        v.profile.kids,
        "t.id IN (SELECT f.title_id FROM progress p JOIN files f ON f.id=p.file_id WHERE p.profile_id=?1)",
        "t.id",
        1000,
        &[&v.profile.id],
    )?;
    taste.apply(&mut cards);
    super::catalog::with_resume(&app, v.profile.id, &mut cards)?;
    let mut out = vec![];
    for c in cards.into_iter().filter(|c| c.status == Some("watching")) {
        let Some(p) = c.resume.as_ref().filter(|p| {
            (p.position > 0.0 || (c.kind == "series" && taste.recent.contains(&c.id)))
                && p.duration > 0.0
                && p.position < p.duration * 0.95
        }) else {
            continue;
        };
        let at = taste.last.get(&c.id).copied().unwrap_or(0);
        out.push(json!({"key":format!("t:{}",c.id),"href":format!("/watch/{}",p.file_id),"position":p.position,"duration":p.duration,"label":p.label,
            "at":at,"art":c.backdrop_small.as_ref().or(c.poster.as_ref()),"card":c}));
    }
    type LinkResume = (i64, String, f64, f64, i64, Option<i64>, Option<i64>);
    let rows:Vec<LinkResume>=app.db.with(|c|{
        let mut st=c.prepare("SELECT l.id,l.ext_id,p.position,p.duration,p.updated_at,l.season,l.episode
          FROM link_progress p JOIN links l ON l.id=p.link_id WHERE p.profile_id=?1 AND l.ext_id IS NOT NULL
          AND p.position>0 AND p.duration>0 AND p.position<p.duration*0.95
          AND NOT EXISTS(SELECT 1 FROM ext_dropped d WHERE d.profile_id=?1 AND d.ext_id=l.ext_id)
          AND NOT EXISTS(SELECT 1 FROM ext_marks m WHERE m.profile_id=?1 AND m.ext_id=l.ext_id AND m.status='watched')
          AND NOT EXISTS(SELECT 1 FROM link_progress newer JOIN links nl ON nl.id=newer.link_id WHERE newer.profile_id=?1 AND nl.ext_id=l.ext_id AND (newer.updated_at>p.updated_at OR(newer.updated_at=p.updated_at AND nl.id>l.id)))
          ORDER BY p.updated_at DESC LIMIT 50")?;
        let rows=st.query_map([v.profile.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?; rows.collect()
    })?;
    for (link, id, position, duration, at, season, episode) in rows {
        if let Ok(ext) = super::discover::ext_card(&app, &v, &id) {
            // Если эта же работа уже продолжена из локального файла, показываем одну карточку.
            if ext.local.is_some_and(|local| out.iter().any(|i| i["key"] == format!("t:{local}"))) {
                continue;
            }
            let art: Option<String> =
                app.db.with(|c| c.query_row("SELECT backdrop FROM ext_titles WHERE id=?", [&id], |r| r.get(0)))?;
            let label = match (season, episode) {
                (Some(s), Some(e)) => format!("{s} сезон, {e} серия"),
                _ => "Фильм".into(),
            };
            out.push(json!({"key":format!("x:{id}"),"href":format!("/watch/link/{link}"),"position":position,"duration":duration,"label":label,"at":at,
                "season":season,"episode":episode,"art":super::img(art).or_else(||ext.poster.clone()),"ext":ext}));
        }
    }
    out.sort_by_key(|i| std::cmp::Reverse(i["at"].as_i64().unwrap_or(0)));
    out.truncate(30);
    Ok(Json(json!(out)))
}
