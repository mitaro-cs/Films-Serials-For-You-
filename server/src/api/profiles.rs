//! Профиль аккаунта: у каждого человека один (с 0.5 выбора «Кто смотрит?» нет). В нём -
//! прогресс, список, оценки, фото и общение; имя совпадает с именем аккаунта.

use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::App;
use rusqlite::params;

pub const AVATARS: i64 = 12;

/// Профиль `id` - свой (фото меняют только у себя).
pub fn own(app: &App, v: &Viewer, id: i64) -> ApiResult<()> {
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
