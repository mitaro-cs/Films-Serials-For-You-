//! Профили внутри аккаунта («Кто смотрит?»): у каждого свои прогресс, список, оценки и
//! подборки. Детский профиль видит только то, что до 12+, и мультфильмы.

use crate::auth::{Profile, Viewer, PROFILE_COLS};
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

pub const MAX_PROFILES: i64 = 6;
pub const AVATARS: i64 = 12;

pub fn list_of(app: &App, user: i64) -> rusqlite::Result<Vec<Profile>> {
    app.db.with(|c| {
        let mut st = c.prepare(&format!("SELECT {PROFILE_COLS} FROM profiles WHERE user_id = ? ORDER BY id"))?;
        let r = st.query_map([user], Profile::from_row)?;
        r.collect()
    })
}

pub async fn list(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "current": v.profile.id, "chosen": v.profile_chosen, "profiles": list_of(&app, v.id)? })))
}

fn check_name(n: &str) -> ApiResult<String> {
    let n = n.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() || n.chars().count() > 24 {
        return Err(ApiError::bad("bad_name", "Имя профиля — от 1 до 24 символов"));
    }
    Ok(n)
}

#[derive(Deserialize)]
pub struct NewProfile {
    name: String,
    avatar: Option<i64>,
    kids: Option<bool>,
}

pub async fn create(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<NewProfile>) -> ApiResult<Json<Profile>> {
    let name = check_name(&b.name)?;
    let avatar = b.avatar.unwrap_or(0).rem_euclid(AVATARS);
    let kids = b.kids.unwrap_or(false);
    let id = app.db.tx(|tx| {
        let n: i64 = tx.query_row("SELECT COUNT(*) FROM profiles WHERE user_id = ?", [v.id], |r| r.get(0))?;
        if n >= MAX_PROFILES {
            return Ok(None);
        }
        tx.execute(
            "INSERT INTO profiles (user_id, name, avatar, kids, created_at) VALUES (?, ?, ?, ?, ?)",
            params![v.id, name, avatar, kids, now_ms()],
        )?;
        Ok(Some(tx.last_insert_rowid()))
    })?;
    let id = id.ok_or_else(|| ApiError::new(StatusCode::CONFLICT, "too_many", "Не больше 6 профилей"))?;
    Ok(Json(Profile { id, name, avatar, kids, autoplay: true }))
}

#[derive(Deserialize)]
pub struct EditProfile {
    name: Option<String>,
    avatar: Option<i64>,
    kids: Option<bool>,
    autoplay: Option<bool>,
}

fn own(app: &App, v: &Viewer, id: i64) -> ApiResult<()> {
    let mine: bool = app.db.with(|c| {
        c.query_row("SELECT EXISTS (SELECT 1 FROM profiles WHERE id = ? AND user_id = ?)", params![id, v.id], |r| {
            r.get(0)
        })
    })?;
    if mine {
        Ok(())
    } else {
        Err(ApiError::not_found())
    }
}

pub async fn edit(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<EditProfile>,
) -> ApiResult<Json<Value>> {
    own(&app, &v, id)?;
    let name = b.name.as_deref().map(check_name).transpose()?;
    app.db.tx(|tx| {
        if let Some(n) = &name {
            tx.execute("UPDATE profiles SET name = ? WHERE id = ?", params![n, id])?;
        }
        if let Some(a) = b.avatar {
            tx.execute("UPDATE profiles SET avatar = ? WHERE id = ?", params![a.rem_euclid(AVATARS), id])?;
        }
        if let Some(k) = b.kids {
            tx.execute("UPDATE profiles SET kids = ? WHERE id = ?", params![k, id])?;
        }
        if let Some(a) = b.autoplay {
            tx.execute("UPDATE profiles SET autoplay = ? WHERE id = ?", params![a, id])?;
        }
        Ok(())
    })?;
    Ok(Json(json!({"ok": true})))
}

pub async fn delete(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    own(&app, &v, id)?;
    let n: i64 =
        app.db.with(|c| c.query_row("SELECT COUNT(*) FROM profiles WHERE user_id = ?", [v.id], |r| r.get(0)))?;
    if n <= 1 {
        return Err(ApiError::bad("last_profile", "Последний профиль удалить нельзя"));
    }
    app.db.with(|c| c.execute("DELETE FROM profiles WHERE id = ?", [id]))?;
    Ok(Json(json!({"ok": true})))
}

/// «Кто смотрит?» — выбор профиля для этой сессии (на этом устройстве).
pub async fn select(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    own(&app, &v, id)?;
    app.db.with(|c| c.execute("UPDATE sessions SET profile_id = ? WHERE hash = ?", params![id, v.token_hash]))?;
    Ok(Json(json!({"ok": true})))
}
