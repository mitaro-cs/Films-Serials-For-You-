//! HTTP API. Права: `Viewer` — любой вошедший, `Manager` — хост или администратор.

mod account;
mod admin;
mod catalog;
mod play;

use crate::App;
use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use rusqlite::Row;
use serde::Serialize;
use std::sync::Arc;
use tower_http::compression::Predicate;

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/api/health", get(account::health))
        .route("/api/site", get(account::site))
        .route("/api/auth/login", post(account::login))
        .route("/api/auth/logout", post(account::logout))
        .route("/api/auth/register", post(account::register))
        .route("/api/invites/{code}", get(account::invite))
        .route("/api/host/enter", get(account::enter))
        .route("/api/me", get(account::me).patch(account::update_me))
        .route("/api/me/password", put(account::password))
        // каталог
        .route("/api/home", get(catalog::home))
        .route("/api/titles", get(catalog::list))
        .route("/api/titles/{id}", get(catalog::title))
        .route("/api/genres", get(catalog::genres))
        .route("/api/list", get(catalog::my_list))
        .route("/api/list/{id}", put(catalog::set_list))
        .route("/img/{name}", get(catalog::image))
        // просмотр
        .route("/api/files/{id}/info", get(play::info))
        .route("/api/files/{id}/raw", get(play::raw))
        .route("/api/files/{id}/download", get(play::download))
        .route("/api/files/{id}/play", get(play::remux))
        .route("/api/files/{id}/subs/{sid}", get(play::subs))
        .route("/api/progress/{id}", put(play::save_progress).delete(play::clear_progress))
        // управление
        .route("/api/admin/status", get(admin::status))
        .route("/api/admin/settings", get(admin::settings).put(admin::save_settings))
        .route("/api/admin/libraries", get(admin::libraries).post(admin::add_library))
        .route("/api/admin/libraries/pick", post(admin::pick_folder))
        .route("/api/admin/libraries/{id}", delete(admin::remove_library))
        .route("/api/admin/scan", post(admin::rescan))
        .route("/api/admin/meta/refresh", post(admin::refresh_all))
        .route("/api/admin/titles", get(admin::titles))
        .route("/api/admin/titles/{id}", patch(admin::edit_title))
        .route("/api/admin/titles/{id}/search", get(admin::search_meta))
        .route("/api/admin/titles/{id}/match", post(admin::match_meta))
        .route("/api/admin/titles/{id}/refresh", post(admin::refresh_title))
        .route("/api/admin/titles/{id}/merge", post(admin::merge_title))
        .route("/api/admin/titles/{id}/image/{which}", put(admin::upload_image))
        .route("/api/admin/files/{id}", patch(admin::edit_file))
        .route("/api/admin/users", get(admin::users).post(admin::create_user))
        .route("/api/admin/users/{id}", patch(admin::edit_user).delete(admin::delete_user))
        .route("/api/admin/invites", get(admin::invites).post(admin::create_invite))
        .route("/api/admin/invites/{code}", delete(admin::delete_invite))
        .route("/api/admin/access", get(admin::access).put(admin::set_access))
        .route("/api/admin/access/login", post(admin::access_login))
        .route("/api/admin/access/logout", post(admin::access_logout))
        .fallback(crate::web::spa)
        .layer(axum::middleware::from_fn(crate::web::headers))
        .layer(
            tower_http::compression::CompressionLayer::new().gzip(true).compress_when(
                tower_http::compression::predicate::SizeAbove::new(1024)
                    .and(tower_http::compression::predicate::NotForContentType::new("video/"))
                    .and(tower_http::compression::predicate::NotForContentType::IMAGES),
            ),
        )
        .with_state(app)
}

/// Произведение в списках и рядах.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub logo: Option<String>,
    pub rating: Option<f64>,
    pub genres: Vec<String>,
    pub overview: Option<String>,
    pub runtime: Option<i64>,
    pub age: Option<String>,
    pub added_at: i64,
    /// Серий у сериала / частей у фильма.
    pub files: i64,
    pub seasons: i64,
}

pub fn img(name: Option<String>) -> Option<String> {
    name.map(|n| format!("/img/{n}"))
}

pub fn json_list(s: Option<String>) -> Vec<String> {
    s.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Видно зрителям: не скрыто и хотя бы один файл на месте.
pub const VISIBLE: &str = "t.hidden = 0 AND EXISTS (SELECT 1 FROM files f WHERE f.title_id = t.id AND f.missing = 0)";

pub const CARD_COLS: &str = "t.id, t.kind, t.title, t.original_title, t.year, t.poster,
    COALESCE(t.backdrop, (SELECT f.still FROM files f WHERE f.title_id = t.id AND f.still IS NOT NULL ORDER BY f.season, f.episode LIMIT 1)),
    t.logo, t.rating, t.genres, t.overview, t.runtime, t.age, t.added_at,
    (SELECT COUNT(*) FROM files f WHERE f.title_id = t.id AND f.missing = 0),
    (SELECT COUNT(DISTINCT f.season) FROM files f WHERE f.title_id = t.id AND f.missing = 0)";

pub fn card(r: &Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: r.get(0)?,
        kind: r.get(1)?,
        title: r.get(2)?,
        original_title: r.get(3)?,
        year: r.get(4)?,
        poster: img(r.get(5)?),
        backdrop: img(r.get(6)?),
        logo: img(r.get(7)?),
        rating: r.get(8)?,
        genres: json_list(r.get(9)?),
        overview: r.get(10)?,
        runtime: r.get(11)?,
        age: r.get(12)?,
        added_at: r.get(13)?,
        files: r.get(14)?,
        seasons: r.get(15)?,
    })
}

pub fn cards(
    app: &App,
    where_sql: &str,
    order: &str,
    limit: i64,
    params: &[&dyn rusqlite::ToSql],
) -> rusqlite::Result<Vec<Card>> {
    let sql =
        format!("SELECT {CARD_COLS} FROM titles t WHERE {VISIBLE} AND ({where_sql}) ORDER BY {order} LIMIT {limit}");
    app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let rows = st.query_map(params, card)?;
        rows.collect()
    })
}
