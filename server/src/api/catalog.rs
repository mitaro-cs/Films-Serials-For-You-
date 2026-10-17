//! Каталог: главная (большой кадр с превью, «Продолжить», Топ-10, подборки), списки, поиск,
//! страница фильма или сериала, «Мой список» и оценки.

use super::{card, cards, cards_by_ids, img, json_list, visible, Card, CARD_COLS};
use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::library::probe::Probe;
use crate::taste::Taste;
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

const DAY: i64 = 86_400_000;

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
    pub added_at: i64,
    pub progress: Option<Progress>,
}

/// Что запускает «Смотреть»: начатое, следующее после досмотренного или первое.
#[derive(Debug, Serialize, Clone, PartialEq)]
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

fn files_of(app: &App, profile: i64, title: i64) -> rusqlite::Result<Vec<FileItem>> {
    app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT f.id, f.season, f.episode, f.episode_end, f.name, f.overview, f.still, f.air_date, f.duration, f.size, f.path,
                    f.added_at, p.position, p.duration, p.done
             FROM files f LEFT JOIN progress p ON p.file_id = f.id AND p.profile_id = ?
             WHERE f.title_id = ? AND f.missing = 0
             ORDER BY f.season, f.episode, f.path",
        )?;
        let rows = st.query_map(params![profile, title], |r| {
            let path: String = r.get(10)?;
            let pos: Option<f64> = r.get(12)?;
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
                added_at: r.get(11)?,
                progress: match pos {
                    Some(position) => Some(Progress { position, duration: r.get(13)?, done: r.get(14)? }),
                    None => None,
                },
            })
        })?;
        rows.collect()
    })
}

/// Последний начатый файл — продолжить; досмотрен — следующий; ничего — первый.
fn next_of(app: &App, profile: i64, title: i64, files: &[FileItem]) -> rusqlite::Result<Option<Next>> {
    let last: Option<i64> = app.db.with(|c| {
        c.query_row(
            "SELECT p.file_id FROM progress p JOIN files f ON f.id = p.file_id
             WHERE p.profile_id = ? AND f.title_id = ? AND f.missing = 0 ORDER BY p.updated_at DESC LIMIT 1",
            params![profile, title],
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

fn continue_watching(app: &App, v: &Viewer, taste: &Taste) -> rusqlite::Result<Vec<ContinueItem>> {
    let mut out = Vec::new();
    for t in taste.recent.iter().take(30) {
        let Some(mut c) = cards(app, v.profile.kids, "t.id = ?", "t.id", 1, &[t])?.pop() else { continue };
        let files = files_of(app, v.profile.id, *t)?;
        let Some(next) = next_of(app, v.profile.id, *t, &files)? else { continue };
        let f = files.iter().find(|f| f.id == next.file_id);
        let whole_done = f.and_then(|f| f.progress.as_ref()).is_some_and(|p| p.done);
        // Досмотренный фильм и последняя серия не висят в «Продолжить».
        if whole_done || (next.position < 1.0 && c.kind == "movie") {
            continue;
        }
        taste.apply(std::slice::from_mut(&mut c));
        out.push(ContinueItem { still: f.and_then(|f| f.still.clone()), card: c, next });
        if out.len() >= 12 {
            break;
        }
    }
    Ok(out)
}

/// Превью в большом кадре: только то, что браузер сыграет сам (без нагрузки на компьютер хоста).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Preview {
    file_id: i64,
    start: f64,
}

fn preview_of(app: &App, title: i64) -> Option<Preview> {
    let (id, path, probe): (i64, String, Option<String>) = app
        .db
        .with(|c| {
            c.query_row(
                "SELECT id, path, probe FROM files WHERE title_id = ? AND missing = 0 ORDER BY season, episode, path LIMIT 1",
                [title],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
        })
        .ok()?;
    let p: Probe = serde_json::from_str(&probe?).ok()?;
    if !crate::stream::direct_ok(std::path::Path::new(&path), &p) {
        return None;
    }
    let d = p.duration.filter(|d| *d > 30.0)?;
    Some(Preview { file_id: id, start: (d * 0.2).min(900.0).floor() })
}

fn row(rows: &mut Vec<Value>, key: &str, title: &str, items: Vec<Card>, style: &str) {
    if !items.is_empty() {
        rows.push(json!({"key": key, "title": title, "items": items, "style": style}));
    }
}

/// Топ-10 за неделю: сколько профилей смотрели (по прогрессу за 7 дней).
fn top10(app: &App, kids: bool) -> rusqlite::Result<Vec<Card>> {
    let ids: Vec<i64> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT f.title_id, COUNT(DISTINCT p.profile_id) AS n, MAX(p.updated_at) AS last
             FROM progress p JOIN files f ON f.id = p.file_id WHERE p.updated_at > ?
             GROUP BY f.title_id ORDER BY n DESC, last DESC LIMIT 15",
        )?;
        let r = st.query_map([now_ms() - 7 * DAY], |r| r.get(0))?;
        r.collect()
    })?;
    let mut out = cards_by_ids(app, kids, &ids)?;
    out.truncate(10);
    Ok(out)
}

pub async fn home(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let kids = v.profile.kids;
    let taste = Taste::load(&app, v.profile.id)?;
    let fresh = |mut items: Vec<Card>| -> Vec<Card> {
        items.retain(|c| !taste.disliked(c.id));
        taste.apply(&mut items);
        items
    };

    // Большой кадр: что ещё не смотрел, ближе по вкусу; каждый день свой порядок.
    let mut hero = fresh(cards(&app, kids, "t.backdrop IS NOT NULL", "t.added_at DESC", 60, &[])?);
    let day = now_ms() / DAY;
    hero.sort_by_key(|c| {
        (
            taste.watched.contains_key(&c.id),
            -(c.match_pct.unwrap_or(0) / 10),
            (c.id.wrapping_mul(2654435761) ^ day).rem_euclid(1000),
        )
    });
    hero.truncate(6);
    let autoplay = v.profile.autoplay;
    let hero: Vec<Value> = hero
        .into_iter()
        .map(|c| {
            let preview = if autoplay { preview_of(&app, c.id) } else { None };
            let mut j = json!(c);
            j["preview"] = json!(preview);
            j
        })
        .collect();

    let mut rows: Vec<Value> = Vec::new();
    let top = fresh(top10(&app, kids)?);
    if top.len() >= 3 {
        row(&mut rows, "top10", "Топ-10 на сайте за неделю", top, "top");
    }
    let list = cards(
        &app,
        kids,
        "t.id IN (SELECT title_id FROM my_list WHERE profile_id = ?)",
        "t.title",
        40,
        &[&v.profile.id],
    )?;
    let mut list = list;
    taste.apply(&mut list);
    row(&mut rows, "list", "Мой список", list, "poster");

    // Новые серии сериалов, которые уже начинали (или просто недавно пополненных).
    let new_eps = fresh(cards(
        &app,
        kids,
        "t.kind = 'series' AND EXISTS (SELECT 1 FROM files f WHERE f.title_id = t.id AND f.missing = 0
           AND f.added_at > ? AND f.added_at > t.added_at + 3600000)",
        "(SELECT MAX(f.added_at) FROM files f WHERE f.title_id = t.id) DESC",
        20,
        &[&(now_ms() - 14 * DAY)],
    )?);
    row(&mut rows, "new-episodes", "Новые серии", new_eps, "poster");

    if let Some(last) = taste.recent.first() {
        if let Some(c) = cards(&app, kids, "t.id = ?", "t.id", 1, &[last])?.pop() {
            let mut sim = fresh(similar(&app, kids, &c)?);
            sim.retain(|s| !taste.watched.contains_key(&s.id));
            row(&mut rows, "because", &format!("Потому что вы смотрели «{}»", c.title), sim, "poster");
        }
    }
    if taste.known() {
        let mut picks = fresh(cards(&app, kids, "1", "t.rating DESC", 400, &[])?);
        picks.retain(|c| c.match_pct.is_some() && !taste.watched.contains_key(&c.id));
        picks.sort_by_key(|c| -c.match_pct.unwrap_or(0));
        picks.truncate(20);
        row(&mut rows, "picks", "Подобрано для вас", picks, "poster");
    }
    row(&mut rows, "new", "Новинки на сайте", fresh(cards(&app, kids, "1", "t.added_at DESC", 20, &[])?), "poster");
    row(
        &mut rows,
        "top",
        "Высокий рейтинг",
        fresh(cards(&app, kids, "t.rating >= 7", "t.rating DESC", 20, &[])?),
        "poster",
    );
    row(
        &mut rows,
        "movies",
        "Фильмы",
        fresh(cards(&app, kids, "t.kind = 'movie'", "t.added_at DESC", 20, &[])?),
        "poster",
    );
    row(
        &mut rows,
        "series",
        "Сериалы",
        fresh(cards(&app, kids, "t.kind = 'series'", "t.added_at DESC", 20, &[])?),
        "poster",
    );
    for (g, _) in top_genres(&app, kids)?.into_iter().take(6) {
        let items = fresh(cards(
            &app,
            kids,
            "EXISTS (SELECT 1 FROM json_each(t.genres) WHERE value = ?)",
            "t.rating DESC",
            20,
            &[&g],
        )?);
        if items.len() >= 4 {
            row(&mut rows, &format!("genre:{g}"), &g, items, "poster");
        }
    }
    Ok(Json(json!({ "hero": hero, "continue": continue_watching(&app, &v, &taste)?, "rows": rows })))
}

fn top_genres(app: &App, kids: bool) -> rusqlite::Result<Vec<(String, i64)>> {
    let sql = format!(
        "SELECT g.value, COUNT(*) AS n FROM titles t, json_each(t.genres) g WHERE {} GROUP BY g.value ORDER BY n DESC, g.value",
        visible(kids)
    );
    app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        r.collect()
    })
}

pub async fn genres(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let g: Vec<Value> = top_genres(&app, v.profile.kids)?
        .into_iter()
        .map(|(name, count)| json!({"name": name, "count": count}))
        .collect();
    Ok(Json(json!(g)))
}

#[derive(Deserialize)]
pub struct ListQuery {
    kind: Option<String>,
    genre: Option<String>,
    sort: Option<String>,
    q: Option<String>,
}

pub async fn list(State(app): State<Arc<App>>, v: Viewer, Query(q): Query<ListQuery>) -> ApiResult<Json<Vec<Card>>> {
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
        v.profile.kids,
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
    let taste = Taste::load(&app, v.profile.id)?;
    taste.apply(&mut items);
    Ok(Json(items))
}

pub async fn title(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let kids = v.profile.kids;
    let mut c = cards(&app, kids, "t.id = ?", "t.id", 1, &[&id])?.pop().ok_or_else(ApiError::not_found)?;
    #[allow(clippy::type_complexity)]
    let (tagline, countries, languages, aka, people, votes, release_date, gallery, imdb_id, provider): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = app.db.with(|db| {
        db.query_row(
            "SELECT tagline, countries, languages, aka, people, votes, release_date, gallery, imdb_id, provider FROM titles WHERE id = ?",
            [id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                ))
            },
        )
    })?;
    let mut people: Vec<crate::meta::Person> = serde_json::from_str(&people).unwrap_or_default();
    for p in &mut people {
        p.photo = img(p.photo.take());
    }
    let files = files_of(&app, v.profile.id, id)?;
    let next = next_of(&app, v.profile.id, id, &files)?;
    let mut seasons: Vec<i32> = files.iter().filter_map(|f| f.season).collect();
    seasons.dedup();
    let taste = Taste::load(&app, v.profile.id)?;
    taste.apply(std::slice::from_mut(&mut c));
    if c.status == Some("watching") {
        c.resume = next.clone();
    }
    let mut gallery: Vec<String> = json_list(gallery).into_iter().map(|g| format!("/img/{g}")).collect();
    if gallery.is_empty() {
        gallery = files.iter().filter_map(|f| f.still.clone()).take(6).collect();
    }
    let mut similar = similar(&app, kids, &c)?;
    similar.retain(|s| !taste.disliked(s.id));
    taste.apply(&mut similar);
    let downloads = app.db.setting_or("site.downloads", "1") == "1" && v.can_download;
    Ok(Json(json!({
        "card": c, "tagline": tagline, "countries": json_list(countries), "languages": json_list(languages),
        "aka": json_list(aka), "people": people, "votes": votes, "releaseDate": release_date, "gallery": gallery,
        "imdbId": imdb_id, "provider": provider, "files": files, "seasons": seasons, "next": next,
        "inList": c.in_list, "similar": similar, "canDownload": downloads,
    })))
}

/// Похожие из своей библиотеки: больше общих жанров, выше рейтинг.
fn similar(app: &App, kids: bool, c: &Card) -> rusqlite::Result<Vec<Card>> {
    if c.genres.is_empty() {
        return Ok(vec![]);
    }
    let all = cards(app, kids, "t.id != ?", "t.rating DESC", 2000, &[&c.id])?;
    let mut scored: Vec<(usize, Card)> = all
        .into_iter()
        .map(|o| (o.genres.iter().filter(|g| c.genres.contains(g)).count() * 2 + usize::from(o.kind == c.kind), o))
        .filter(|(s, _)| *s >= 2)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.rating.partial_cmp(&a.1.rating).unwrap_or(std::cmp::Ordering::Equal)));
    Ok(scored.into_iter().take(12).map(|(_, c)| c).collect())
}

pub async fn my_list(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Vec<Card>>> {
    let sql = format!(
        "SELECT {CARD_COLS} FROM my_list l JOIN titles t ON t.id = l.title_id WHERE l.profile_id = ? AND {} ORDER BY l.added_at DESC",
        visible(v.profile.kids)
    );
    let mut items = app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let r = st.query_map([v.profile.id], card)?;
        r.collect::<rusqlite::Result<Vec<_>>>()
    })?;
    Taste::load(&app, v.profile.id)?.apply(&mut items);
    Ok(Json(items))
}

#[derive(Deserialize)]
pub struct LibraryQuery {
    /// movies, series, new; иначе — всё.
    tab: Option<String>,
    /// want, watching, watched; иначе — всё.
    status: Option<String>,
}

/// Новое — последние добавленные (сколько бы давно ни было).
const NEW_LIMIT: usize = 60;

/// Каталог с билетами: вкладка, фильтр по статусу, числа у фильтров и «где остановился».
pub async fn library(State(app): State<Arc<App>>, v: Viewer, Query(q): Query<LibraryQuery>) -> ApiResult<Json<Value>> {
    let kind = match q.tab.as_deref() {
        Some("movies") => Some("movie"),
        Some("series") => Some("series"),
        _ => None,
    };
    let mut items =
        cards(&app, v.profile.kids, "(?1 IS NULL OR t.kind = ?1)", "t.added_at DESC, t.id DESC", 5000, &[&kind])?;
    if q.tab.as_deref() == Some("new") {
        items.truncate(NEW_LIMIT);
    }
    let taste = Taste::load(&app, v.profile.id)?;
    taste.apply(&mut items);
    let count = |s: &str| items.iter().filter(|c| c.status == Some(s)).count();
    let counts = json!({
        "all": items.len(), "want": count("want"), "watching": count("watching"), "watched": count("watched"),
    });
    if let Some(s) = q.status.as_deref().filter(|s| matches!(*s, "want" | "watching" | "watched")) {
        items.retain(|c| c.status == Some(s));
        match s {
            "watched" => items.sort_by_key(|c| std::cmp::Reverse(c.watched_at)),
            "watching" => items.sort_by_key(|c| taste.recent.iter().position(|id| *id == c.id)),
            _ => {}
        }
    }
    with_resume(&app, v.profile.id, &mut items)?;
    Ok(Json(json!({"items": items, "counts": counts})))
}

/// «Где остановился» у начатого: серия и место, с которого продолжит «Смотреть».
pub fn with_resume(app: &App, profile: i64, items: &mut [Card]) -> rusqlite::Result<()> {
    for c in items.iter_mut().filter(|c| c.status == Some("watching")) {
        let files = files_of(app, profile, c.id)?;
        c.resume = next_of(app, profile, c.id, &files)?;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct WatchedBody {
    value: bool,
}

/// «Просмотрено» вручную (билет с датой) или снять отметку — тогда и прогресс с начала.
pub async fn set_watched(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<WatchedBody>,
) -> ApiResult<Json<Value>> {
    cards(&app, v.profile.kids, "t.id = ?", "t.id", 1, &[&id])?.pop().ok_or_else(ApiError::not_found)?;
    app.db.tx(|c| {
        if b.value {
            c.execute(
                "INSERT INTO watched (profile_id, title_id, at) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
                params![v.profile.id, id, now_ms()],
            )?;
            c.execute("DELETE FROM my_list WHERE profile_id = ? AND title_id = ?", params![v.profile.id, id])?;
        } else {
            c.execute("DELETE FROM watched WHERE profile_id = ? AND title_id = ?", params![v.profile.id, id])?;
            c.execute(
                "DELETE FROM progress WHERE profile_id = ? AND file_id IN (SELECT id FROM files WHERE title_id = ?)",
                params![v.profile.id, id],
            )?;
        }
        Ok(())
    })?;
    Ok(Json(json!({"ok": true})))
}

/// После сохранения прогресса: фильм досмотрен или все серии сериала досмотрены — «Просмотрено».
pub fn mark_if_finished(app: &App, profile: i64, title: i64) -> rusqlite::Result<bool> {
    app.db.with(|c| {
        let left: i64 = c.query_row(
            "SELECT COUNT(*) FROM files f LEFT JOIN progress p ON p.file_id = f.id AND p.profile_id = ?
             WHERE f.title_id = ? AND f.missing = 0 AND COALESCE(p.done, 0) = 0",
            params![profile, title],
            |r| r.get(0),
        )?;
        if left > 0 {
            return Ok(false);
        }
        c.execute(
            "INSERT INTO watched (profile_id, title_id, at) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
            params![profile, title, now_ms()],
        )?;
        c.execute("DELETE FROM my_list WHERE profile_id = ? AND title_id = ?", params![profile, title])?;
        Ok(true)
    })
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
                "INSERT OR IGNORE INTO my_list (profile_id, title_id, added_at) SELECT ?, id, ? FROM titles WHERE id = ?",
                params![v.profile.id, now_ms(), id],
            )
        } else {
            c.execute("DELETE FROM my_list WHERE profile_id = ? AND title_id = ?", params![v.profile.id, id])
        }
    })?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct RatingBody {
    /// -1 — «не для меня», 1 — «нравится», 2 — «обожаю», 0 — снять оценку.
    value: i32,
}

pub async fn set_rating(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<RatingBody>,
) -> ApiResult<Json<Value>> {
    if !matches!(b.value, -1..=2) {
        return Err(ApiError::bad("bad_value", "Оценка: -1, 0, 1 или 2"));
    }
    app.db.with(|c| {
        if b.value == 0 {
            c.execute("DELETE FROM ratings WHERE profile_id = ? AND title_id = ?", params![v.profile.id, id])
        } else {
            c.execute(
                "INSERT INTO ratings (profile_id, title_id, value, updated_at) SELECT ?, id, ?, ? FROM titles WHERE id = ?
                 ON CONFLICT(profile_id, title_id) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![v.profile.id, b.value, now_ms(), id],
            )
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
