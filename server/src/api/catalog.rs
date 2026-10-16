//! Каталог: главная (большой кадр, «Продолжить», ряды), списки, страница фильма или сериала.

use super::{card, cards, img, json_list, Card, CARD_COLS, VISIBLE};
use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::util::{norm, now_ms};
use crate::App;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub position: f64,
    pub duration: f64,
    pub done: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    pub id: i64,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub episode_end: Option<i32>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub still: Option<String>,
    pub air_date: Option<String>,
    pub duration: Option<f64>,
    pub size: i64,
    pub file_name: String,
    pub progress: Option<Progress>,
}

/// Что запускает «Смотреть»: начатое, следующее после досмотренного или первое.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Next {
    pub file_id: i64,
    pub label: String,
    pub position: f64,
    pub duration: f64,
}

pub fn episode_label(season: Option<i32>, episode: Option<i32>, name: Option<&str>) -> String {
    match (season, episode) {
        (Some(s), Some(e)) => match name {
            Some(n) => format!("{s} сезон, {e} серия · {n}"),
            None => format!("{s} сезон, {e} серия"),
        },
        _ => String::new(),
    }
}

fn files_of(app: &App, user: i64, title: i64) -> rusqlite::Result<Vec<FileItem>> {
    app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT f.id, f.season, f.episode, f.episode_end, f.name, f.overview, f.still, f.air_date, f.duration, f.size, f.path,
                    p.position, p.duration, p.done
             FROM files f LEFT JOIN progress p ON p.file_id = f.id AND p.user_id = ?
             WHERE f.title_id = ? AND f.missing = 0
             ORDER BY f.season, f.episode, f.path",
        )?;
        let rows = st.query_map(params![user, title], |r| {
            let path: String = r.get(10)?;
            let pos: Option<f64> = r.get(11)?;
            Ok(FileItem {
                id: r.get(0)?,
                season: r.get(1)?,
                episode: r.get(2)?,
                episode_end: r.get(3)?,
                name: r.get(4)?,
                overview: r.get(5)?,
                still: img(r.get(6)?),
                air_date: r.get(7)?,
                duration: r.get(8)?,
                size: r.get(9)?,
                file_name: std::path::Path::new(&path).file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string(),
                progress: match pos {
                    Some(position) => Some(Progress { position, duration: r.get(12)?, done: r.get(13)? }),
                    None => None,
                },
            })
        })?;
        rows.collect()
    })
}

/// Последний начатый файл — продолжить; досмотрен — следующий; ничего — первый.
fn next_of(app: &App, user: i64, title: i64, files: &[FileItem]) -> rusqlite::Result<Option<Next>> {
    let last: Option<i64> = app.db.with(|c| {
        c.query_row(
            "SELECT p.file_id FROM progress p JOIN files f ON f.id = p.file_id
             WHERE p.user_id = ? AND f.title_id = ? AND f.missing = 0 ORDER BY p.updated_at DESC LIMIT 1",
            params![user, title],
            |r| r.get(0),
        )
        .optional()
    })?;
    let idx = last.and_then(|id| files.iter().position(|f| f.id == id));
    let pick = match idx {
        Some(i) if files[i].progress.as_ref().is_some_and(|p| p.done) => files.get(i + 1).or(files.get(i)),
        Some(i) => files.get(i),
        None => files.first(),
    };
    Ok(pick.map(|f| {
        let p = f.progress.as_ref().filter(|p| !p.done);
        Next {
            file_id: f.id,
            label: episode_label(f.season, f.episode, f.name.as_deref()),
            position: p.map(|p| p.position).unwrap_or(0.0),
            duration: p.map(|p| p.duration).or(f.duration).unwrap_or(0.0),
        }
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueItem {
    #[serde(flatten)]
    pub card: Card,
    pub next: Next,
    pub still: Option<String>,
}

fn continue_watching(app: &App, user: i64) -> rusqlite::Result<Vec<ContinueItem>> {
    let recent: Vec<i64> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT f.title_id, MAX(p.updated_at) AS at FROM progress p JOIN files f ON f.id = p.file_id
             WHERE p.user_id = ? GROUP BY f.title_id ORDER BY at DESC LIMIT 30",
        )?;
        let r = st.query_map([user], |r| r.get(0))?;
        r.collect()
    })?;
    let mut out = Vec::new();
    for t in recent {
        let Some(c) = cards(app, "t.id = ?", "t.id", 1, &[&t])?.pop() else {
            continue;
        };
        let files = files_of(app, user, t)?;
        let Some(next) = next_of(app, user, t, &files)? else {
            continue;
        };
        let f = files.iter().find(|f| f.id == next.file_id);
        let whole_done = f.and_then(|f| f.progress.as_ref()).is_some_and(|p| p.done);
        // Досмотренный фильм и последняя серия не висят в «Продолжить».
        if whole_done || (next.position < 1.0 && c.kind == "movie") {
            continue;
        }
        out.push(ContinueItem { still: f.and_then(|f| f.still.clone()), card: c, next });
        if out.len() >= 12 {
            break;
        }
    }
    Ok(out)
}

pub async fn home(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let mut hero = cards(&app, "t.backdrop IS NOT NULL", "t.added_at DESC", 40, &[])?;
    // Каждый день — свой порядок, но в течение дня стабильный.
    let day = now_ms() / 86_400_000;
    hero.sort_by_key(|c| (c.id.wrapping_mul(2654435761) ^ day).rem_euclid(1000));
    hero.truncate(6);

    let mut rows: Vec<Value> = Vec::new();
    let mut row = |key: &str, title: &str, items: Vec<Card>| {
        if !items.is_empty() {
            rows.push(json!({"key": key, "title": title, "items": items}));
        }
    };
    row("new", "Новинки на сайте", cards(&app, "1", "t.added_at DESC", 20, &[])?);
    let list = cards(&app, "t.id IN (SELECT title_id FROM my_list WHERE user_id = ?)", "t.title", 30, &[&v.id])?;
    row("list", "Мой список", list);
    row("top", "Высокий рейтинг", cards(&app, "t.rating >= 7", "t.rating DESC", 20, &[])?);
    row("movies", "Фильмы", cards(&app, "t.kind = 'movie'", "t.added_at DESC", 20, &[])?);
    row("series", "Сериалы", cards(&app, "t.kind = 'series'", "t.added_at DESC", 20, &[])?);
    for (g, _) in top_genres(&app)?.into_iter().take(5) {
        let items =
            cards(&app, "EXISTS (SELECT 1 FROM json_each(t.genres) WHERE value = ?)", "t.rating DESC", 20, &[&g])?;
        if items.len() >= 4 {
            row(&format!("genre:{g}"), &g, items);
        }
    }
    Ok(Json(json!({ "hero": hero, "continue": continue_watching(&app, v.id)?, "rows": rows })))
}

fn top_genres(app: &App) -> rusqlite::Result<Vec<(String, i64)>> {
    let sql = format!(
        "SELECT g.value, COUNT(*) AS n FROM titles t, json_each(t.genres) g WHERE {VISIBLE} GROUP BY g.value ORDER BY n DESC, g.value"
    );
    app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        r.collect()
    })
}

pub async fn genres(State(app): State<Arc<App>>, _v: Viewer) -> ApiResult<Json<Value>> {
    let g: Vec<Value> =
        top_genres(&app)?.into_iter().map(|(name, count)| json!({"name": name, "count": count})).collect();
    Ok(Json(json!(g)))
}

#[derive(Deserialize)]
pub struct ListQuery {
    kind: Option<String>,
    genre: Option<String>,
    sort: Option<String>,
    q: Option<String>,
}

pub async fn list(State(app): State<Arc<App>>, _v: Viewer, Query(q): Query<ListQuery>) -> ApiResult<Json<Vec<Card>>> {
    let kind = q.kind.filter(|k| k == "movie" || k == "series");
    let genre = q.genre.filter(|g| !g.is_empty());
    let order = match q.sort.as_deref() {
        Some("rating") => "t.rating IS NULL, t.rating DESC",
        Some("year") => "t.year IS NULL, t.year DESC",
        Some("title") => "t.title COLLATE NOCASE",
        _ => "t.added_at DESC",
    };
    let mut items = cards(
        &app,
        "(?1 IS NULL OR t.kind = ?1) AND (?2 IS NULL OR EXISTS (SELECT 1 FROM json_each(t.genres) WHERE value = ?2))",
        order,
        5000,
        &[&kind, &genre],
    )?;
    if let Some(query) = q.q.as_deref().map(norm).filter(|s| !s.is_empty()) {
        // Поиск в памяти: регистр и «ё» в кириллице SQLite сам не сравнивает.
        let words: Vec<&str> = query.split(' ').collect();
        items.retain(|c| {
            let hay = norm(&format!(
                "{} {} {}",
                c.title,
                c.original_title.as_deref().unwrap_or(""),
                c.year.map(|y| y.to_string()).unwrap_or_default()
            ));
            words.iter().all(|w| hay.contains(w))
        });
    }
    Ok(Json(items))
}

pub async fn title(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let c = cards(&app, "t.id = ?", "t.id", 1, &[&id])?.pop().ok_or_else(ApiError::not_found)?;
    #[allow(clippy::type_complexity)]
    let (tagline, countries, languages, aka, people, votes, release_date, gallery, imdb_id, provider): (
        Option<String>, Option<String>, Option<String>, Option<String>, String, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>,
    ) = app.db.with(|db| {
        db.query_row(
            "SELECT tagline, countries, languages, aka, people, votes, release_date, gallery, imdb_id, provider FROM titles WHERE id = ?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)),
        )
    })?;
    let mut people: Vec<crate::meta::Person> = serde_json::from_str(&people).unwrap_or_default();
    for p in &mut people {
        p.photo = img(p.photo.take());
    }
    let files = files_of(&app, v.id, id)?;
    let next = next_of(&app, v.id, id, &files)?;
    let mut seasons: Vec<i32> = files.iter().filter_map(|f| f.season).collect();
    seasons.dedup();
    let in_list: bool = app.db.with(|db| {
        db.query_row(
            "SELECT EXISTS (SELECT 1 FROM my_list WHERE user_id = ? AND title_id = ?)",
            params![v.id, id],
            |r| r.get(0),
        )
    })?;
    let mut gallery: Vec<String> = json_list(gallery).into_iter().map(|g| format!("/img/{g}")).collect();
    if gallery.is_empty() {
        gallery = files.iter().filter_map(|f| f.still.clone()).take(6).collect();
    }
    let similar = similar(&app, &c)?;
    let downloads = app.db.setting_or("site.downloads", "1") == "1" && v.can_download;
    Ok(Json(json!({
        "card": c, "tagline": tagline, "countries": json_list(countries), "languages": json_list(languages),
        "aka": json_list(aka), "people": people, "votes": votes, "releaseDate": release_date, "gallery": gallery,
        "imdbId": imdb_id, "provider": provider, "files": files, "seasons": seasons, "next": next,
        "inList": in_list, "similar": similar, "canDownload": downloads,
    })))
}

/// Похожие из своей библиотеки: больше общих жанров, выше рейтинг.
fn similar(app: &App, c: &Card) -> rusqlite::Result<Vec<Card>> {
    if c.genres.is_empty() {
        return Ok(vec![]);
    }
    let all = cards(app, "t.id != ?", "t.rating DESC", 2000, &[&c.id])?;
    let mut scored: Vec<(usize, Card)> = all
        .into_iter()
        .map(|o| (o.genres.iter().filter(|g| c.genres.contains(g)).count() + usize::from(o.kind == c.kind), o))
        .filter(|(s, o)| *s > usize::from(o.kind == c.kind))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.rating.partial_cmp(&a.1.rating).unwrap_or(std::cmp::Ordering::Equal)));
    Ok(scored.into_iter().take(12).map(|(_, c)| c).collect())
}

pub async fn my_list(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Vec<Card>>> {
    let sql = format!(
        "SELECT {CARD_COLS} FROM my_list l JOIN titles t ON t.id = l.title_id WHERE l.user_id = ? AND {VISIBLE} ORDER BY l.added_at DESC"
    );
    let items = app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let r = st.query_map([v.id], card)?;
        r.collect::<rusqlite::Result<Vec<_>>>()
    })?;
    Ok(Json(items))
}

#[derive(Deserialize)]
pub struct ListBody {
    value: bool,
}

pub async fn set_list(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<ListBody>,
) -> ApiResult<Json<Value>> {
    app.db.with(|c| {
        if b.value {
            c.execute(
                "INSERT OR IGNORE INTO my_list (user_id, title_id, added_at) SELECT ?, id, ? FROM titles WHERE id = ?",
                params![v.id, now_ms(), id],
            )
        } else {
            c.execute("DELETE FROM my_list WHERE user_id = ? AND title_id = ?", params![v.id, id])
        }
    })?;
    Ok(Json(json!({"ok": true})))
}

pub async fn image(State(app): State<Arc<App>>, _v: Viewer, Path(name): Path<String>) -> ApiResult<Response> {
    let path = app.images.path(&name).ok_or_else(ApiError::not_found)?;
    let bytes = tokio::fs::read(&path).await?;
    let mime = match name.rsplit('.').next() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "private, max-age=31536000, immutable")], bytes)
        .into_response())
}
