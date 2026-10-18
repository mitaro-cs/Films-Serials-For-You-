//! Совместные списки: «смотрим вместе» с друзьями. Список видят и меняют все участники:
//! добавляют фильмы, отмечают «посмотрели». Позвать можно только своего друга; создатель
//! удаляет список, остальные — выходят из него.

use super::social::{person, relation, text, title_visible, Person, PERSON_COLS, SOCIAL};
use super::{card, visible, CARD_COLS, CARD_LEN};
use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::util::now_ms;
use crate::App;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

const MAX_LISTS: i64 = 30;
const MAX_MEMBERS: usize = 12;
const MAX_ITEMS: i64 = 500;

fn member_of(app: &App, v: &Viewer, list: i64) -> ApiResult<i64> {
    let owner: Option<i64> = app.db.with(|c| {
        use rusqlite::OptionalExtension;
        c.query_row(
            "SELECT l.owner_id FROM lists l JOIN list_members m ON m.list_id = l.id WHERE l.id = ? AND m.profile_id = ?",
            params![list, v.profile.id],
            |r| r.get(0),
        )
        .optional()
    })?;
    owner.ok_or_else(ApiError::not_found)
}

fn members(app: &App, list: i64) -> rusqlite::Result<Vec<Person>> {
    app.db.with(|c| {
        let mut st = c.prepare(&format!(
            "SELECT {PERSON_COLS} FROM list_members m JOIN profiles p ON p.id = m.profile_id
             WHERE m.list_id = ? ORDER BY m.rowid"
        ))?;
        let r = st.query_map([list], |r| person(r, 0))?;
        r.collect()
    })
}

fn friend(app: &App, v: &Viewer, other: i64) -> ApiResult<()> {
    let ok: bool = app.db.with(|c| {
        c.query_row(&format!("SELECT EXISTS (SELECT 1 FROM profiles p WHERE p.id = ? AND {SOCIAL})"), [other], |r| {
            r.get(0)
        })
    })?;
    if !ok || relation(app, v.profile.id, other)? != "friend" {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "not_friends", "Позвать в список можно только друга"));
    }
    Ok(())
}

fn name_of(s: &str) -> ApiResult<String> {
    let n = text(s, 60, "Название")?;
    if n.is_empty() {
        return Err(ApiError::bad("bad_name", "Назовите список"));
    }
    Ok(n)
}

/// Мои списки: участники, сколько фильмов и сколько уже посмотрели, кадры для обложки.
pub async fn lists(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let rows: Vec<(i64, String, i64, i64, i64)> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT l.id, l.name, l.owner_id,
               (SELECT COUNT(*) FROM list_items i WHERE i.list_id = l.id),
               (SELECT COUNT(*) FROM list_items i WHERE i.list_id = l.id AND i.watched_at IS NOT NULL)
             FROM lists l JOIN list_members m ON m.list_id = l.id AND m.profile_id = ?
             ORDER BY COALESCE((SELECT MAX(i.added_at) FROM list_items i WHERE i.list_id = l.id), l.created_at) DESC",
        )?;
        let r = st.query_map([v.profile.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        r.collect()
    })?;
    let mut out = Vec::new();
    for (id, name, owner, count, watched) in rows {
        let art: Vec<String> = app.db.with(|c| {
            let mut st = c.prepare(
                "SELECT COALESCE(t.backdrop_sm, t.backdrop, t.poster_sm, t.poster) FROM list_items i
                 JOIN titles t ON t.id = i.title_id WHERE i.list_id = ? AND COALESCE(t.backdrop, t.poster) IS NOT NULL
                 ORDER BY i.watched_at IS NOT NULL, i.added_at DESC LIMIT 4",
            )?;
            let r = st.query_map([id], |r| r.get::<_, String>(0))?;
            r.collect()
        })?;
        out.push(json!({
            "id": id, "name": name, "mine": owner == v.profile.id, "count": count, "watched": watched,
            "members": members(&app, id)?,
            "art": art.into_iter().filter_map(|a| super::img(Some(a))).collect::<Vec<_>>(),
        }));
    }
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
pub struct NewList {
    name: String,
    #[serde(default)]
    members: Vec<i64>,
}

pub async fn create(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<NewList>) -> ApiResult<Json<Value>> {
    let name = name_of(&b.name)?;
    let mut others: Vec<i64> = b.members.into_iter().filter(|m| *m != v.profile.id).collect();
    others.sort_unstable();
    others.dedup();
    if others.len() + 1 > MAX_MEMBERS {
        return Err(ApiError::bad("too_many", format!("В списке — не больше {MAX_MEMBERS} человек")));
    }
    for m in &others {
        friend(&app, &v, *m)?;
    }
    let n: i64 =
        app.db.with(|c| c.query_row("SELECT COUNT(*) FROM lists WHERE owner_id = ?", [v.profile.id], |r| r.get(0)))?;
    if n >= MAX_LISTS {
        return Err(ApiError::bad("too_many", format!("Не больше {MAX_LISTS} своих списков")));
    }
    let id = app.db.tx(|tx| {
        tx.execute(
            "INSERT INTO lists (name, owner_id, created_at) VALUES (?, ?, ?)",
            params![name, v.profile.id, now_ms()],
        )?;
        let id = tx.last_insert_rowid();
        for m in std::iter::once(v.profile.id).chain(others.iter().copied()) {
            tx.execute("INSERT INTO list_members (list_id, profile_id) VALUES (?, ?)", params![id, m])?;
        }
        Ok(id)
    })?;
    Ok(Json(json!({"id": id})))
}

/// Список целиком: участники и фильмы (сначала не просмотренные, новые сверху).
pub async fn list(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let owner = member_of(&app, &v, id)?;
    let name: String = app.db.with(|c| c.query_row("SELECT name FROM lists WHERE id = ?", [id], |r| r.get(0)))?;
    let sql = format!(
        "SELECT {CARD_COLS}, i.added_at, i.watched_at, {} FROM list_items i JOIN titles t ON t.id = i.title_id
         LEFT JOIN profiles p ON p.id = i.added_by
         WHERE i.list_id = ? AND {} ORDER BY i.watched_at IS NOT NULL, i.watched_at DESC, i.added_at DESC",
        "COALESCE(p.id, 0), COALESCE(p.name, ''), COALESCE(p.avatar, 0), p.photo",
        visible(v.profile.kids)
    );
    let n = CARD_LEN;
    let items: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let r = st.query_map([id], |r| {
            let added_by = person(r, n + 2)?;
            Ok(json!({
                "card": card(r)?,
                "addedAt": r.get::<_, i64>(n)?,
                "watchedAt": r.get::<_, Option<i64>>(n + 1)?,
                "addedBy": (added_by.id != 0).then_some(added_by),
            }))
        })?;
        r.collect()
    })?;
    Ok(Json(json!({
        "id": id, "name": name, "mine": owner == v.profile.id, "members": members(&app, id)?, "items": items,
    })))
}

#[derive(Deserialize)]
pub struct EditList {
    name: String,
}

pub async fn rename(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<EditList>,
) -> ApiResult<Json<Value>> {
    member_of(&app, &v, id)?;
    let name = name_of(&b.name)?;
    app.db.with(|c| c.execute("UPDATE lists SET name = ? WHERE id = ?", params![name, id]))?;
    Ok(Json(json!({"ok": true})))
}

/// Создатель удаляет список, участник — выходит из него.
pub async fn delete(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let owner = member_of(&app, &v, id)?;
    app.db.with(|c| {
        if owner == v.profile.id {
            c.execute("DELETE FROM lists WHERE id = ?", [id])
        } else {
            c.execute("DELETE FROM list_members WHERE list_id = ? AND profile_id = ?", params![id, v.profile.id])
        }
    })?;
    Ok(Json(json!({"ok": true, "left": owner != v.profile.id})))
}

#[derive(Deserialize)]
pub struct MemberBody {
    id: i64,
}

pub async fn invite(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<MemberBody>,
) -> ApiResult<Json<Value>> {
    member_of(&app, &v, id)?;
    friend(&app, &v, b.id)?;
    if members(&app, id)?.len() >= MAX_MEMBERS {
        return Err(ApiError::bad("too_many", format!("В списке — не больше {MAX_MEMBERS} человек")));
    }
    app.db.with(|c| {
        c.execute("INSERT OR IGNORE INTO list_members (list_id, profile_id) VALUES (?, ?)", params![id, b.id])
    })?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct Flag {
    value: bool,
}

/// Добавить фильм в список или убрать.
pub async fn item(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path((id, title)): Path<(i64, i64)>,
    Json(b): Json<Flag>,
) -> ApiResult<Json<Value>> {
    member_of(&app, &v, id)?;
    title_visible(&app, &v, title)?;
    if b.value {
        let n: i64 =
            app.db.with(|c| c.query_row("SELECT COUNT(*) FROM list_items WHERE list_id = ?", [id], |r| r.get(0)))?;
        if n >= MAX_ITEMS {
            return Err(ApiError::bad("too_many", format!("В списке — не больше {MAX_ITEMS} фильмов")));
        }
        app.db.with(|c| {
            c.execute(
                "INSERT OR IGNORE INTO list_items (list_id, title_id, added_by, added_at) VALUES (?, ?, ?, ?)",
                params![id, title, v.profile.id, now_ms()],
            )
        })?;
    } else {
        app.db.with(|c| c.execute("DELETE FROM list_items WHERE list_id = ? AND title_id = ?", params![id, title]))?;
    }
    Ok(Json(json!({"ok": true})))
}

/// «Посмотрели вместе» — или снять отметку.
pub async fn watched(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path((id, title)): Path<(i64, i64)>,
    Json(b): Json<Flag>,
) -> ApiResult<Json<Value>> {
    member_of(&app, &v, id)?;
    let n = app.db.with(|c| {
        c.execute(
            "UPDATE list_items SET watched_at = ? WHERE list_id = ? AND title_id = ?",
            params![b.value.then(now_ms), id, title],
        )
    })?;
    if n == 0 {
        return Err(ApiError::not_found());
    }
    Ok(Json(json!({"ok": true})))
}

/// Для страницы фильма: мои списки и есть ли в них этот фильм.
pub async fn of_title(State(app): State<Arc<App>>, v: Viewer, Path(title): Path<i64>) -> ApiResult<Json<Value>> {
    title_visible(&app, &v, title)?;
    let rows: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT l.id, l.name, EXISTS (SELECT 1 FROM list_items i WHERE i.list_id = l.id AND i.title_id = ?1)
             FROM lists l JOIN list_members m ON m.list_id = l.id AND m.profile_id = ?2 ORDER BY l.name",
        )?;
        let r = st.query_map(params![title, v.profile.id], |r| {
            Ok(json!({"id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "has": r.get::<_, bool>(2)?}))
        })?;
        r.collect()
    })?;
    Ok(Json(json!(rows)))
}
