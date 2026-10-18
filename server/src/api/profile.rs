//! Личная страница: статистика (часы, фильмы, серии, любимые жанры, по месяцам) и оформление —
//! обложка, свой цвет, «о себе», любимые фильмы (с сервиса и из ленты IMDb).

use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::App;
use axum::extract::State;
use axum::Json;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

const MONTH: i64 = 30 * 86_400_000;

/// Статистика человека. `now` — для подсчёта по месяцам (последние 12).
pub fn stats(app: &App, id: i64, now: i64) -> ApiResult<Value> {
    let (minutes, movies, episodes, series, reviews, friends, outside): (f64, i64, i64, i64, i64, i64, i64) =
        app.db.with(|c| {
            c.query_row(
                "SELECT
                   COALESCE((SELECT SUM(MIN(p.position, p.duration)) FROM progress p WHERE p.profile_id = ?1), 0) / 60.0,
                   (SELECT COUNT(*) FROM watched w JOIN titles t ON t.id = w.title_id WHERE w.profile_id = ?1 AND t.kind = 'movie'),
                   (SELECT COUNT(*) FROM progress p JOIN files f ON f.id = p.file_id JOIN titles t ON t.id = f.title_id
                      WHERE p.profile_id = ?1 AND p.done = 1 AND t.kind = 'series'),
                   (SELECT COUNT(DISTINCT f.title_id) FROM progress p JOIN files f ON f.id = p.file_id JOIN titles t ON t.id = f.title_id
                      WHERE p.profile_id = ?1 AND t.kind = 'series'),
                   (SELECT COUNT(*) FROM reviews WHERE profile_id = ?1),
                   (SELECT COUNT(*) FROM friends WHERE status = 'accepted' AND (a = ?1 OR b = ?1)),
                   (SELECT COUNT(*) FROM ext_marks WHERE profile_id = ?1 AND status = 'watched')",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )
        })?;
    // Жанры — по всему, что смотрел (с сервиса) и отметил просмотренным в ленте.
    let genre_lists: Vec<String> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT t.genres FROM titles t WHERE t.id IN (
               SELECT f.title_id FROM progress p JOIN files f ON f.id = p.file_id WHERE p.profile_id = ?1
               UNION SELECT title_id FROM watched WHERE profile_id = ?1)
             UNION ALL
             SELECT e.genres FROM ext_titles e JOIN ext_marks m ON m.ext_id = e.id AND m.profile_id = ?1 AND m.status = 'watched'",
        )?;
        let r = st.query_map([id], |r| r.get::<_, Option<String>>(0))?;
        r.map(|g| g.map(|g| g.unwrap_or_default())).collect()
    })?;
    let mut genres: HashMap<String, i64> = HashMap::new();
    for g in genre_lists.iter().flat_map(|s| super::json_list(Some(s.clone()))) {
        *genres.entry(g).or_default() += 1;
    }
    let mut genres: Vec<(String, i64)> = genres.into_iter().collect();
    genres.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    genres.truncate(6);
    // По месяцам: досмотренное (фильмы и серии) за последние 12 месяцев, старое — слева.
    let done: Vec<i64> = app.db.with(|c| {
        let mut st =
            c.prepare("SELECT updated_at FROM progress WHERE profile_id = ? AND done = 1 AND updated_at > ?")?;
        let r = st.query_map(params![id, now - 12 * MONTH], |r| r.get(0))?;
        r.collect()
    })?;
    let mut months = [0i64; 12];
    for at in done {
        let ago = ((now - at) / MONTH).clamp(0, 11) as usize;
        months[11 - ago] += 1;
    }
    Ok(json!({
        "hours": (minutes / 60.0 * 10.0).round() / 10.0,
        "movies": movies, "episodes": episodes, "series": series, "reviews": reviews, "friends": friends,
        "outside": outside,
        "genres": genres.into_iter().map(|(g, n)| json!({"genre": g, "count": n})).collect::<Vec<_>>(),
        "months": months,
    }))
}

/// Оформление профиля с любимыми фильмами (карточки).
pub fn decor(app: &App, v: &Viewer, id: i64) -> ApiResult<Value> {
    let (cover, accent, bio, favs): (Option<String>, Option<String>, Option<String>, String) = app.db.with(|c| {
        c.query_row("SELECT cover, accent, bio, favorites FROM profiles WHERE id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
    })?;
    let mut favorites = Vec::new();
    for f in super::json_list(Some(favs)) {
        if let Some(t) = f.strip_prefix("t:").and_then(|t| t.parse::<i64>().ok()) {
            if let Some(c) = super::cards_by_ids(app, v.profile.kids, &[t])?.pop() {
                favorites.push(json!({"key": f, "card": c}));
            }
        } else if let Some(e) = f.strip_prefix("x:") {
            if let Ok(c) = super::discover::ext_card(app, v, e) {
                favorites.push(json!({"key": f, "ext": c}));
            }
        }
    }
    Ok(json!({"cover": super::img(cover), "accent": accent, "bio": bio, "favorites": favorites}))
}

#[derive(Deserialize)]
pub struct DecorBody {
    bio: Option<String>,
    accent: Option<String>,
    favorites: Option<Vec<String>>,
}

fn valid_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub async fn set_decor(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<DecorBody>) -> ApiResult<Json<Value>> {
    let me = v.profile.id;
    if let Some(bio) = &b.bio {
        let bio = bio.trim();
        if bio.chars().count() > 300 {
            return Err(ApiError::bad("too_long", "«О себе» — не длиннее 300 символов"));
        }
        app.db.with(|c| {
            c.execute("UPDATE profiles SET bio = ? WHERE id = ?", params![(!bio.is_empty()).then_some(bio), me])
        })?;
    }
    if let Some(a) = &b.accent {
        let a = a.trim().to_lowercase();
        if !a.is_empty() && !valid_color(&a) {
            return Err(ApiError::bad("bad_color", "Цвет — в виде #ff8800"));
        }
        app.db.with(|c| {
            c.execute("UPDATE profiles SET accent = ? WHERE id = ?", params![(!a.is_empty()).then_some(a), me])
        })?;
    }
    if let Some(f) = &b.favorites {
        let mut keep: Vec<String> = Vec::new();
        for k in f {
            let ok = k.strip_prefix("t:").is_some_and(|t| t.parse::<i64>().is_ok())
                || k.strip_prefix("x:").is_some_and(crate::discover::valid_id);
            if ok && !keep.contains(k) {
                keep.push(k.clone());
            }
        }
        if keep.len() > 6 {
            return Err(ApiError::bad("too_many", "Любимых — не больше 6"));
        }
        let s = serde_json::to_string(&keep).unwrap_or_else(|_| "[]".into());
        app.db.with(|c| c.execute("UPDATE profiles SET favorites = ? WHERE id = ?", params![s, me]))?;
    }
    Ok(Json(json!({"ok": true})))
}

/// Обложка профиля: картинка, уменьшенная до 1600 px по ширине.
pub async fn set_cover(State(app): State<Arc<App>>, v: Viewer, body: axum::body::Bytes) -> ApiResult<Json<Value>> {
    if body.len() > crate::images::MAX_IMAGE {
        return Err(ApiError::bad("too_big", "Картинка больше 15 МБ"));
    }
    if crate::images::sniff(&body).is_none() {
        return Err(ApiError::bad("bad_image", "Нужна картинка: JPEG, PNG или WebP"));
    }
    let a = app.clone();
    let raw = body.to_vec();
    let name = tokio::task::spawn_blocking(move || a.images.save(&raw))
        .await
        .map_err(ApiError::internal)?
        .map_err(|e| ApiError::bad("bad_image", e.to_string()))?;
    let ffmpeg = app.tools.read().ffmpeg.clone();
    let name = match (ffmpeg, app.images.path(&name)) {
        (Some(ff), Some(src)) => match crate::library::probe::scale_image(&ff, &src, 1600).await {
            Ok(b) => app.images.save(&b).unwrap_or(name),
            Err(_) => name,
        },
        _ => name,
    };
    app.db.with(|c| c.execute("UPDATE profiles SET cover = ? WHERE id = ?", params![name, v.profile.id]))?;
    Ok(Json(json!({"cover": format!("/img/{name}")})))
}

pub async fn delete_cover(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    app.db.with(|c| c.execute("UPDATE profiles SET cover = NULL WHERE id = ?", [v.profile.id]))?;
    Ok(Json(json!({"ok": true})))
}
