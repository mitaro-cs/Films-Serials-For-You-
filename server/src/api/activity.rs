//! Кто что смотрит: лента «Друзья смотрят», «Смотрят друзья» на странице фильма и «Сейчас смотрит»
//! в профиле. Видно себе и друзьям - и только если человек не выключил «Показывать, что я смотрю».

use super::social::{person, Person, PERSON_COLS, SOCIAL};
use super::{cards_by_ids, discover::ext_cards};
use crate::auth::Viewer;
use crate::error::ApiResult;
use crate::taste::Taste;
use crate::util::now_ms;
use crate::App;
use axum::extract::State;
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::sync::Arc;

/// Начатое давнее двух месяцев - уже не «смотрит сейчас».
const FRESH: i64 = 60 * 86_400_000;

/// Друзья (принятые), которые показывают, что смотрят.
fn open_friends(app: &App, me: i64) -> rusqlite::Result<Vec<Person>> {
    app.db.with(|c| {
        let mut st = c.prepare(&format!(
            "SELECT {PERSON_COLS} FROM friends f JOIN profiles p ON p.id = CASE WHEN f.a = ?1 THEN f.b ELSE f.a END
             WHERE (f.a = ?1 OR f.b = ?1) AND f.status = 'accepted' AND p.show_activity = 1 AND {SOCIAL}"
        ))?;
        let r = st.query_map([me], |r| person(r, 0))?;
        r.collect()
    })
}

/// Видно ли `me`, что смотрит `id`: сам или друг, который не скрыл.
pub fn visible_to(app: &App, me: i64, id: i64) -> rusqlite::Result<bool> {
    if me == id {
        return Ok(true);
    }
    app.db.with(|c| {
        c.query_row(
            "SELECT EXISTS (SELECT 1 FROM friends f JOIN profiles p ON p.id = ?2
               WHERE f.status = 'accepted' AND ((f.a = ?1 AND f.b = ?2) OR (f.a = ?2 AND f.b = ?1)) AND p.show_activity = 1)",
            params![me, id],
            |r| r.get(0),
        )
    })
}

/// Что смотрит: с сервиса (или из ленты), когда последний раз и где остановился.
pub struct Now {
    pub at: i64,
    pub title: Option<i64>,
    pub ext: Option<String>,
    pub label: String,
}

fn ep(season: Option<i32>, episode: Option<i32>) -> String {
    match (season, episode) {
        (Some(s), Some(e)) => format!("{s} сезон, {e} серия"),
        _ => String::new(),
    }
}

/// Что человек смотрит сейчас: начатое с сервиса (не брошенное, не досмотренное, за два месяца),
/// отмеченное «Смотрю» вручную и сериалы из ленты. Свежее - первым.
pub fn watching_now(app: &App, profile: i64) -> rusqlite::Result<Vec<Now>> {
    let t = Taste::marks(app, profile)?;
    let now = now_ms();
    let mut out: Vec<Now> = Vec::new();
    for id in &t.recent {
        let at = t.last.get(id).copied().unwrap_or(0);
        if t.status(*id) != Some("watching") || now - at > FRESH {
            continue;
        }
        let last: Option<(Option<i32>, Option<i32>)> = app.db.with(|c| {
            c.query_row(
                "SELECT f.season, f.episode FROM progress p JOIN files f ON f.id = p.file_id
                 WHERE p.profile_id = ? AND f.title_id = ? ORDER BY p.updated_at DESC LIMIT 1",
                params![profile, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
        })?;
        let label = last.map(|(s, e)| ep(s, e)).unwrap_or_default();
        out.push(Now { at, title: Some(*id), ext: None, label });
    }
    let manual: Vec<(i64, i64)> = app.db.with(|c| {
        let mut st = c.prepare("SELECT title_id, at FROM watching WHERE profile_id = ?")?;
        let r = st.query_map([profile], |r| Ok((r.get(0)?, r.get(1)?)))?;
        r.collect()
    })?;
    for (id, at) in manual {
        if !t.seen.contains_key(&id) && !out.iter().any(|n| n.title == Some(id)) {
            out.push(Now { at, title: Some(id), ext: None, label: String::new() });
        }
    }
    let ext: Vec<(String, i64, Option<i32>, Option<i32>)> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT m.ext_id, MAX(m.at, COALESCE((SELECT MAX(s.at) FROM ext_seen s WHERE s.profile_id = m.profile_id AND s.ext_id = m.ext_id), 0)),
                    (SELECT s.season FROM ext_seen s WHERE s.profile_id = m.profile_id AND s.ext_id = m.ext_id ORDER BY s.season DESC, s.episode DESC LIMIT 1),
                    (SELECT s.episode FROM ext_seen s WHERE s.profile_id = m.profile_id AND s.ext_id = m.ext_id ORDER BY s.season DESC, s.episode DESC LIMIT 1)
             FROM ext_marks m WHERE m.profile_id = ? AND m.status = 'watching'",
        )?;
        let r = st.query_map([profile], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        r.collect()
    })?;
    for (id, at, s, e) in ext {
        let label = match ep(s, e) {
            l if l.is_empty() => l,
            l => format!("посмотрел {l}"),
        };
        out.push(Now { at, title: None, ext: Some(id), label });
    }
    out.sort_by_key(|n| std::cmp::Reverse(n.at));
    out.truncate(24);
    Ok(out)
}

/// Карточки для того, кто смотрит ленту: с его отметками и «совпадением».
pub fn render(app: &App, v: &Viewer, items: &[Now]) -> ApiResult<Vec<Value>> {
    let taste = Taste::load(app, v.profile.id)?;
    let mut out = Vec::new();
    for n in items {
        if let Some(id) = n.title {
            let Some(mut c) = cards_by_ids(app, v.profile.kids, &[id])?.pop() else { continue };
            taste.apply(std::slice::from_mut(&mut c));
            out.push(json!({"at": n.at, "label": n.label, "card": c}));
        } else if let Some(e) = &n.ext {
            if let Some(c) = ext_cards(app, v, "e.id = ?2", "e.id", &[e])?.pop() {
                out.push(json!({"at": n.at, "label": n.label, "ext": c}));
            }
        }
    }
    Ok(out)
}

/// «Друзья смотрят»: что сейчас смотрит каждый друг, свежее - первым.
pub async fn friends(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    if v.profile.kids {
        return Ok(Json(json!([])));
    }
    let mut all: Vec<(Now, Value)> = Vec::new();
    for p in open_friends(&app, v.profile.id)? {
        let who = json!(p);
        for n in watching_now(&app, p.id)?.into_iter().take(6) {
            all.push((n, who.clone()));
        }
    }
    all.sort_by_key(|(n, _)| std::cmp::Reverse(n.at));
    all.truncate(40);
    let (nows, people): (Vec<Now>, Vec<Value>) = all.into_iter().unzip();
    let mut out = Vec::new();
    // Карточки - по одной (у каждой свой человек): render сохраняет порядок, пропуская невидимые.
    for (n, who) in nows.iter().zip(people) {
        if let Some(mut item) = render(&app, &v, std::slice::from_ref(n))?.pop() {
            item["person"] = who;
            out.push(item);
        }
    }
    Ok(Json(json!(out)))
}

/// Друзья и их отметки у фильма: «смотрит», «просмотрел», «хочет» (для страницы фильма).
pub fn of_title(app: &App, v: &Viewer, title: Option<i64>, ext: Option<&str>) -> ApiResult<Vec<Value>> {
    if v.profile.kids {
        return Ok(vec![]);
    }
    // Тот же фильм с другой стороны: у фильма с сервиса - его id IMDb, у фильма из ленты - наш.
    let (title, ext): (Option<i64>, Option<String>) = match (title, ext) {
        (Some(t), _) => (
            Some(t),
            app.db
                .with(|c| c.query_row("SELECT imdb_id FROM titles WHERE id = ?", [t], |r| r.get(0)).optional())?
                .flatten(),
        ),
        (None, Some(e)) => (
            app.db.with(|c| {
                c.query_row(
                    &format!("SELECT t.id FROM titles t WHERE t.imdb_id = ? AND {}", super::VISIBLE),
                    [e],
                    |r| r.get(0),
                )
                .optional()
            })?,
            Some(e.to_string()),
        ),
        _ => (None, None),
    };
    let mut out = Vec::new();
    for p in open_friends(app, v.profile.id)? {
        let local = match title {
            Some(t) => Taste::marks(app, p.id)?.status(t),
            None => None,
        };
        let outside: Option<String> = match &ext {
            Some(e) => app.db.with(|c| {
                c.query_row("SELECT status FROM ext_marks WHERE profile_id = ? AND ext_id = ?", params![p.id, e], |r| {
                    r.get(0)
                })
                .optional()
            })?,
            None => None,
        };
        let rank = |s: &str| match s {
            "watching" => 3,
            "watched" => 2,
            _ => 1,
        };
        let status = match (local, outside.as_deref()) {
            (Some(a), Some(b)) => Some(if rank(a) >= rank(b) { a.to_string() } else { b.to_string() }),
            (Some(a), None) => Some(a.to_string()),
            (None, Some(b)) => Some(b.to_string()),
            _ => None,
        };
        if let Some(s) = status {
            out.push(json!({"person": p, "status": s}));
        }
    }
    let rank = |v: &Value| match v["status"].as_str() {
        Some("watching") => 0,
        Some("watched") => 1,
        _ => 2,
    };
    out.sort_by_key(rank);
    Ok(out)
}
