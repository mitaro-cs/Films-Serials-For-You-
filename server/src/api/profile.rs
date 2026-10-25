//! Личная страница: статистика (часы, фильмы, серии, любимые жанры, по месяцам) и оформление -
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

/// Статистика человека. `now` - для подсчёта по месяцам (последние 12).
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
    // Источники по ссылке раньше вовсе не попадали в статистику. Несколько источников одной
    // серии считаем один раз; фильм, уже учтённый через локальный файл, не дублируем.
    let external_minutes: f64 = app.db.with(|c| {
        c.query_row(
            "SELECT COALESCE(SUM(pos),0)/60.0 FROM (
          SELECT MAX(MIN(p.position,p.duration)) pos FROM link_progress p JOIN links l ON l.id=p.link_id
          WHERE p.profile_id=?1 AND NOT EXISTS(SELECT 1 FROM progress x JOIN files f ON f.id=x.file_id
            WHERE x.profile_id=?1 AND f.title_id=l.title_id)
          GROUP BY COALESCE(l.ext_id,'t:'||l.title_id,'l:'||l.id),l.season,l.episode)",
            [id],
            |r| r.get(0),
        )
    })?;
    let (external_movies, external_episodes, external_series): (i64,i64,i64) = app.db.with(|c| c.query_row(
        "SELECT
          (SELECT COUNT(*) FROM ext_marks m JOIN ext_titles e ON e.id=m.ext_id WHERE m.profile_id=?1 AND m.status='watched' AND e.kind='movie'
            AND NOT EXISTS(SELECT 1 FROM watched w JOIN titles t ON t.id=w.title_id WHERE w.profile_id=?1 AND t.imdb_id=e.id)),
          (SELECT COUNT(*) FROM ext_seen s WHERE s.profile_id=?1 AND NOT EXISTS(SELECT 1 FROM progress p JOIN files f ON f.id=p.file_id JOIN titles t ON t.id=f.title_id
            WHERE p.profile_id=?1 AND p.done=1 AND t.imdb_id=s.ext_id AND f.season=s.season AND f.episode=s.episode)),
          (SELECT COUNT(DISTINCT s.ext_id) FROM ext_seen s WHERE s.profile_id=?1 AND NOT EXISTS(SELECT 1 FROM progress p JOIN files f ON f.id=p.file_id JOIN titles t ON t.id=f.title_id
            WHERE p.profile_id=?1 AND t.imdb_id=s.ext_id))", [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))))?;
    // Жанры - по всему, что смотрел (с сервиса) и отметил просмотренным в ленте.
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
    // Режиссёры того, что смотрел, - чаще всего первыми (подсказки для «любимых режиссёров»).
    let peoples: Vec<String> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT t.people FROM titles t WHERE t.id IN (
               SELECT f.title_id FROM progress p JOIN files f ON f.id = p.file_id WHERE p.profile_id = ?1
               UNION SELECT title_id FROM watched WHERE profile_id = ?1)",
        )?;
        let r = st.query_map([id], |r| r.get(0))?;
        r.collect()
    })?;
    let mut dirs: HashMap<String, i64> = HashMap::new();
    for p in &peoples {
        if let Ok(Value::Array(list)) = serde_json::from_str::<Value>(p) {
            for x in list.iter().filter(|x| x["job"] == "director") {
                if let Some(n) = x["name"].as_str() {
                    *dirs.entry(n.to_string()).or_default() += 1;
                }
            }
        }
    }
    let mut dirs: Vec<(String, i64)> = dirs.into_iter().collect();
    dirs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    dirs.truncate(10);
    let mut genres: HashMap<String, i64> = HashMap::new();
    for g in genre_lists.iter().flat_map(|s| super::json_list(Some(s.clone()))) {
        *genres.entry(g).or_default() += 1;
    }
    let mut genres: Vec<(String, i64)> = genres.into_iter().collect();
    genres.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    genres.truncate(6);
    // По месяцам: досмотренное (фильмы и серии) за последние 12 месяцев, старое - слева.
    let done: Vec<i64> = app.db.with(|c| {
        let mut st = c.prepare("SELECT MAX(at) FROM (
          SELECT COALESCE(t.imdb_id,'t:'||t.id)||':'||COALESCE(f.season,0)||':'||COALESCE(f.episode,0) item,p.updated_at at
            FROM progress p JOIN files f ON f.id=p.file_id JOIN titles t ON t.id=f.title_id WHERE p.profile_id=?1 AND p.done=1
          UNION ALL SELECT COALESCE(t.imdb_id,'t:'||t.id)||':0:0',w.at FROM watched w JOIN titles t ON t.id=w.title_id WHERE w.profile_id=?1 AND t.kind='movie'
          UNION ALL SELECT s.ext_id||':'||s.season||':'||s.episode,s.at FROM ext_seen s WHERE s.profile_id=?1
          UNION ALL SELECT m.ext_id||':0:0',m.at FROM ext_marks m JOIN ext_titles e ON e.id=m.ext_id WHERE m.profile_id=?1 AND m.status='watched' AND e.kind='movie'
        ) GROUP BY item HAVING MAX(at)>?2")?;
        let r = st.query_map(params![id, now - 13 * MONTH], |r| r.get(0))?;
        r.collect()
    })?;
    let mut months = [0i64; 12];
    let month_index = |at: i64| -> rusqlite::Result<i32> {
        app.db.with(|c| c.query_row(
        "SELECT CAST(strftime('%Y',?1/1000,'unixepoch') AS INTEGER)*12 + CAST(strftime('%m',?1/1000,'unixepoch') AS INTEGER)-1", [at], |r| r.get(0)))
    };
    let current = month_index(now)?;
    for at in done {
        let ago = current - month_index(at)?;
        if (0..12).contains(&ago) {
            months[11 - ago as usize] += 1;
        }
    }
    let labels = (0..12)
        .map(|i| {
            let m = current - 11 + i;
            format!("{:04}-{:02}", m / 12, m % 12 + 1)
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "hours": ((minutes + external_minutes) / 60.0 * 10.0).round() / 10.0,
        "movies": movies + external_movies, "episodes": episodes + external_episodes, "series": series + external_series, "reviews": reviews, "friends": friends,
        "outside": outside,
        "genres": genres.into_iter().map(|(g, n)| json!({"genre": g, "count": n})).collect::<Vec<_>>(),
        "months": months,
        "monthLabels": labels,
        "directors": dirs.into_iter().map(|(n, c)| json!({"name": n, "count": c})).collect::<Vec<_>>(),
    }))
}

/// Оформление профиля с любимыми фильмами (карточки).
pub fn decor(app: &App, v: &Viewer, id: i64) -> ApiResult<Value> {
    type Row = (Option<String>, Option<String>, Option<String>, String, String, String, bool);
    let (cover, accent, bio, favs, directors, kind, show): Row = app.db.with(|c| {
        c.query_row(
            "SELECT cover, accent, bio, favorites, directors, cover_kind, show_activity FROM profiles WHERE id = ?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
    })?;
    // Лента обложки - постеры просмотренного (с сервиса и из Обзора), свежие первыми.
    let ribbon: Vec<String> = app.db.with(|c| {
        let mut st = c.prepare(&format!(
            "SELECT p FROM (
               SELECT COALESCE(t.poster_sm, t.poster) p, w.at FROM watched w JOIN titles t ON t.id = w.title_id
                 WHERE w.profile_id = ?1 AND {}
               UNION ALL
               SELECT e.poster, m.at FROM ext_marks m JOIN ext_titles e ON e.id = m.ext_id
                 WHERE m.profile_id = ?1 AND m.status = 'watched')
             WHERE p IS NOT NULL AND p != '' ORDER BY at DESC LIMIT 30",
            super::visible(v.profile.kids)
        ))?;
        let r = st.query_map([id], |r| r.get::<_, String>(0))?;
        r.collect()
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
    Ok(json!({
        "cover": super::img(cover), "coverKind": kind, "accent": accent, "bio": bio, "favorites": favorites,
        "directors": super::json_list(Some(directors.clone())), "showActivity": show,
        // Фото режиссёров - что уже известно (без запросов к IMDb; недостающее ищет `directors::warm`).
        "directorsInfo": super::json_list(Some(directors)).into_iter().map(|n| {
            let photo = crate::directors::cached(app, &n).and_then(|c| c.1);
            json!({"name": n, "photo": super::img(photo)})
        }).collect::<Vec<_>>(),
        "ribbon": ribbon.into_iter().filter_map(|n| super::img(Some(n))).collect::<Vec<_>>(),
    }))
}

#[derive(Deserialize)]
pub struct DecorBody {
    bio: Option<String>,
    directors: Option<Vec<String>>,
    #[serde(rename = "coverKind")]
    cover_kind: Option<String>,
    accent: Option<String>,
    favorites: Option<Vec<String>>,
    /// Показывать друзьям, что смотрю.
    #[serde(rename = "showActivity")]
    show_activity: Option<bool>,
}

/// Большое избранное, отдельно фильмы и сериалы.
pub const MAX_FAVORITES: usize = 100;

/// Фильм или сериал у ключа любимого («t:12» - с сервиса, «x:tt…» - из ленты).
fn fav_kind(app: &App, key: &str) -> Option<String> {
    let (sql, id) = if let Some(t) = key.strip_prefix("t:") {
        ("SELECT kind FROM titles WHERE id = ?", t)
    } else {
        ("SELECT kind FROM ext_titles WHERE id = ?", key.strip_prefix("x:")?)
    };
    app.db.with(|c| c.query_row(sql, [id], |r| r.get(0))).ok()
}

/// Слишком много любимых одного вида - ошибка с понятным текстом.
fn check_favorites(app: &App, list: &[String]) -> ApiResult<()> {
    // Неизвестное (фильма уже нет) - считаем фильмом: иначе любимых можно было бы набрать сколько угодно.
    let kinds: Vec<Option<String>> = list.iter().map(|k| fav_kind(app, k).or(Some("movie".into()))).collect();
    for (kind, what) in [("movie", "фильмов"), ("series", "сериалов")] {
        if kinds.iter().filter(|k| k.as_deref() == Some(kind)).count() > MAX_FAVORITES {
            return Err(ApiError::bad("too_many", format!("В избранном можно сохранить до {MAX_FAVORITES} {what}")));
        }
    }
    Ok(())
}

fn valid_key(k: &str) -> bool {
    k.strip_prefix("t:").is_some_and(|t| t.parse::<i64>().is_ok())
        || k.strip_prefix("x:").is_some_and(crate::discover::valid_id)
}

#[derive(Deserialize)]
pub struct FavBody {
    key: String,
    value: bool,
}

/// Один фильм в любимые или из них (с его страницы). Шестой фильм (или сериал) - нельзя: сначала убрать один.
pub async fn set_favorite(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<FavBody>) -> ApiResult<Json<Value>> {
    if !valid_key(&b.key) {
        return Err(ApiError::bad("bad_key", "Неизвестный фильм"));
    }
    let me = v.profile.id;
    let raw: String =
        app.db.with(|c| c.query_row("SELECT favorites FROM profiles WHERE id = ?", [me], |r| r.get(0)))?;
    let mut list = super::json_list(Some(raw));
    list.retain(|k| k != &b.key);
    if b.value {
        list.push(b.key.clone());
        check_favorites(&app, &list)?;
    }
    let s = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
    app.db.with(|c| c.execute("UPDATE profiles SET favorites = ? WHERE id = ?", params![s, me]))?;
    Ok(Json(json!({"favorites": list})))
}

fn valid_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub async fn set_decor(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<DecorBody>) -> ApiResult<Json<Value>> {
    let me = v.profile.id;
    if let Some(bio) = &b.bio {
        let bio = bio.trim();
        if bio.chars().count() > 300 {
            return Err(ApiError::bad("too_long", "«О себе» - не длиннее 300 символов"));
        }
        app.db.with(|c| {
            c.execute("UPDATE profiles SET bio = ? WHERE id = ?", params![(!bio.is_empty()).then_some(bio), me])
        })?;
    }
    if let Some(a) = &b.accent {
        let a = a.trim().to_lowercase();
        if !a.is_empty() && !valid_color(&a) {
            return Err(ApiError::bad("bad_color", "Цвет - в виде #ff8800"));
        }
        app.db.with(|c| {
            c.execute("UPDATE profiles SET accent = ? WHERE id = ?", params![(!a.is_empty()).then_some(a), me])
        })?;
    }
    if let Some(d) = &b.directors {
        let mut keep: Vec<String> = Vec::new();
        for n in d.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
            if n.chars().count() > 80 {
                return Err(ApiError::bad("too_long", "Имя режиссёра - не длиннее 80 символов"));
            }
            if !keep.iter().any(|k| k.to_lowercase() == n.to_lowercase()) {
                keep.push(n.to_string());
            }
        }
        if keep.len() > MAX_FAVORITES {
            return Err(ApiError::bad("too_many", format!("Любимых режиссёров - не больше {MAX_FAVORITES}")));
        }
        let s = serde_json::to_string(&keep).unwrap_or_else(|_| "[]".into());
        app.db.with(|c| c.execute("UPDATE profiles SET directors = ? WHERE id = ?", params![s, me]))?;
    }
    if let Some(k) = &b.cover_kind {
        if k != "ribbon" && k != "image" {
            return Err(ApiError::bad("bad_value", "Обложка - лента или своя картинка"));
        }
        app.db.with(|c| c.execute("UPDATE profiles SET cover_kind = ? WHERE id = ?", params![k, me]))?;
    }
    if let Some(s) = b.show_activity {
        app.db.with(|c| c.execute("UPDATE profiles SET show_activity = ? WHERE id = ?", params![s, me]))?;
    }
    if let Some(f) = &b.favorites {
        let mut keep: Vec<String> = Vec::new();
        for k in f {
            if valid_key(k) && !keep.contains(k) {
                keep.push(k.clone());
            }
        }
        check_favorites(&app, &keep)?;
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
    app.db.with(|c| {
        c.execute("UPDATE profiles SET cover = ?, cover_kind = 'image' WHERE id = ?", params![name, v.profile.id])
    })?;
    Ok(Json(json!({"cover": format!("/img/{name}")})))
}

pub async fn delete_cover(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    app.db
        .with(|c| c.execute("UPDATE profiles SET cover = NULL, cover_kind = 'ribbon' WHERE id = ?", [v.profile.id]))?;
    Ok(Json(json!({"ok": true})))
}
