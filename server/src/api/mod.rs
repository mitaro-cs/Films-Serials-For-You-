//! HTTP API. Права: `Viewer` — любой вошедший, `Manager` — хост или администратор.

pub(crate) mod account;
mod admin;
mod catalog;
mod play;
mod profiles;
mod social;

pub use play::busy;

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
        .route("/api/titles/{id}/rating", put(catalog::set_rating))
        .route("/api/profiles", get(profiles::list).post(profiles::create))
        .route("/api/profiles/{id}", patch(profiles::edit).delete(profiles::delete))
        .route("/api/profiles/{id}/select", post(profiles::select))
        .route("/api/list/{id}", put(catalog::set_list))
        .route("/api/library", get(catalog::library))
        .route("/api/titles/{id}/watched", put(catalog::set_watched))
        .route("/img/{name}", get(catalog::image))
        // люди
        .route("/api/titles/{id}/reviews", get(social::reviews).post(social::add_review))
        .route("/api/reviews/{id}", patch(social::edit_review).delete(social::delete_review))
        .route("/api/reviews/{id}/like", put(social::like_review))
        .route("/api/members", get(social::members))
        .route("/api/members/{id}", get(social::member))
        .route("/api/friends/{id}", post(social::befriend).delete(social::unfriend))
        .route("/api/chats", get(social::chats))
        .route("/api/chats/{id}", get(social::chat).post(social::send))
        .route("/api/badges", get(social::badges))
        .route(
            "/api/profiles/{id}/photo",
            put(social::set_photo)
                .delete(social::delete_photo)
                .layer(axum::extract::DefaultBodyLimit::max(crate::images::MAX_IMAGE + 1024)),
        )
        // просмотр
        .route("/api/files/{id}/info", get(play::info))
        .route("/api/files/{id}/raw", get(play::raw))
        .route("/api/files/{id}/download", get(play::download))
        .route("/api/files/{id}/play", get(play::remux))
        .route("/api/files/{id}/subs/{sid}", get(play::subs))
        .route("/api/files/{id}/hls/master.m3u8", get(play::hls_master))
        .route("/api/files/{id}/hls/{h}/index.m3u8", get(play::hls_variant))
        .route("/api/files/{id}/hls/{h}/{seg}", get(play::hls_segment))
        .route("/api/files/{id}/trickplay/{sheet}", get(play::trickplay))
        .route("/api/files/{id}/markers", put(play::set_markers))
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
        .route(
            "/api/admin/titles/{id}/image/{which}",
            put(admin::upload_image).layer(axum::extract::DefaultBodyLimit::max(crate::images::MAX_IMAGE + 1024)),
        )
        .route("/api/admin/files/{id}", patch(admin::edit_file))
        .route("/api/admin/users", get(admin::users).post(admin::create_user))
        .route("/api/admin/users/{id}", patch(admin::edit_user).delete(admin::delete_user))
        .route("/api/admin/invites", get(admin::invites).post(admin::create_invite))
        .route("/api/admin/invites/{code}", delete(admin::delete_invite))
        .route("/api/admin/access", get(admin::access).put(admin::set_access))
        .route("/api/admin/access/login", post(admin::access_login))
        .route("/api/admin/access/logout", post(admin::access_logout))
        .route("/api/admin/access/cloudpub/login", post(admin::cloudpub_login))
        .route("/api/admin/access/cloudpub/logout", post(admin::cloudpub_logout))
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
    /// Постер для рядов (уменьшенный, если есть).
    pub poster: Option<String>,
    pub poster_large: Option<String>,
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
    /// Для профиля: в «Моём списке», своя оценка (-1, 1, 2), «совпадение» в процентах.
    pub in_list: bool,
    pub my_rating: Option<i32>,
    pub match_pct: Option<i32>,
    /// Для билета: первая страна, режиссёр, цвет низа карточки по постеру.
    pub country: Option<String>,
    pub director: Option<String>,
    pub accent: Option<String>,
    /// Статус профиля: want («Хочу посмотреть»), watching, watched; None — ничего.
    pub status: Option<&'static str>,
    /// Когда отмечено «Просмотрено» (для белого билета).
    pub watched_at: Option<i64>,
    /// Где остановился: что запустит «Продолжить» и с какого места.
    pub resume: Option<catalog::Next>,
}

pub fn img(name: Option<String>) -> Option<String> {
    name.map(|n| format!("/img/{n}"))
}

pub fn json_list(s: Option<String>) -> Vec<String> {
    s.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Видно зрителям: не скрыто и хотя бы один файл на месте.
pub const VISIBLE: &str = "t.hidden = 0 AND EXISTS (SELECT 1 FROM files f WHERE f.title_id = t.id AND f.missing = 0)";

/// Детский профиль: возраст до 12+ или мультфильмы и семейное.
pub const KIDS: &str = "((t.age_min IS NOT NULL AND t.age_min <= 12) OR EXISTS (SELECT 1 FROM json_each(t.genres)
    WHERE value IN ('Мультфильм', 'Семейный', 'Анимация', 'Детский', 'Мультсериал', 'Family', 'Animation')))";

pub fn visible(kids: bool) -> String {
    if kids {
        format!("{VISIBLE} AND {KIDS}")
    } else {
        VISIBLE.to_string()
    }
}

pub const CARD_COLS: &str = "t.id, t.kind, t.title, t.original_title, t.year, COALESCE(t.poster_sm, t.poster), t.poster,
    COALESCE(t.backdrop, (SELECT f.still FROM files f WHERE f.title_id = t.id AND f.still IS NOT NULL ORDER BY f.season, f.episode LIMIT 1)),
    t.logo, t.rating, t.genres, t.overview,
    -- Без описания из интернета длительность фильма — по самому файлу.
    COALESCE(t.runtime, CASE WHEN t.kind = 'movie' THEN (SELECT CAST(ROUND(SUM(f.duration) / 60.0) AS INTEGER)
      FROM files f WHERE f.title_id = t.id AND f.missing = 0 AND f.duration > 0) END),
    t.age, t.added_at,
    (SELECT COUNT(*) FROM files f WHERE f.title_id = t.id AND f.missing = 0),
    (SELECT COUNT(DISTINCT f.season) FROM files f WHERE f.title_id = t.id AND f.missing = 0),
    json_extract(t.countries, '$[0]'),
    (SELECT json_extract(value, '$.name') FROM json_each(t.people) WHERE json_extract(value, '$.job') = 'director' LIMIT 1),
    t.accent";

pub fn card(r: &Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: r.get(0)?,
        kind: r.get(1)?,
        title: r.get(2)?,
        original_title: r.get(3)?,
        year: r.get(4)?,
        poster: img(r.get(5)?),
        poster_large: img(r.get(6)?),
        backdrop: img(r.get(7)?),
        logo: img(r.get(8)?),
        rating: r.get(9)?,
        genres: json_list(r.get(10)?),
        overview: r.get(11)?,
        runtime: r.get(12)?,
        age: r.get(13)?,
        added_at: r.get(14)?,
        files: r.get(15)?,
        seasons: r.get(16)?,
        in_list: false,
        my_rating: None,
        match_pct: None,
        country: r.get(17)?,
        director: r.get(18)?,
        accent: r.get(19)?,
        status: None,
        watched_at: None,
        resume: None,
    })
}

pub fn cards(
    app: &App,
    kids: bool,
    where_sql: &str,
    order: &str,
    limit: i64,
    params: &[&dyn rusqlite::ToSql],
) -> rusqlite::Result<Vec<Card>> {
    let sql = format!(
        "SELECT {CARD_COLS} FROM titles t WHERE {} AND ({where_sql}) ORDER BY {order} LIMIT {limit}",
        visible(kids)
    );
    app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let rows = st.query_map(params, card)?;
        rows.collect()
    })
}

/// Карточки по списку id в том же порядке (Топ-10, история).
pub fn cards_by_ids(app: &App, kids: bool, ids: &[i64]) -> rusqlite::Result<Vec<Card>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let list = ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let mut out = cards(app, kids, &format!("t.id IN ({list})"), "t.id", ids.len() as i64, &[])?;
    out.sort_by_key(|c| ids.iter().position(|i| *i == c.id));
    Ok(out)
}
