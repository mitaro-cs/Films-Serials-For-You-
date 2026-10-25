//! Совместный просмотр (`crate::rooms`): создать комнату на файл, войти (поток событий),
//! управлять местом в видео, чат, реакции и позвать друга сообщением.

use super::social::{person, PERSON_COLS, SOCIAL};
use super::visible;
use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::util::now_ms;
use crate::App;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

/// Реакции поверх видео - только эти.
const REACTIONS: &[&str] = &["❤️", "😂", "😮", "😢", "🔥", "👏", "😱", "👍"];

fn me(app: &App, v: &Viewer) -> ApiResult<Value> {
    let p = app.db.with(|c| {
        c.query_row(&format!("SELECT {PERSON_COLS} FROM profiles p WHERE p.id = ?"), [v.profile.id], |r| person(r, 0))
    })?;
    Ok(json!(p))
}

/// Файл виден этому зрителю - его фильм (id).
fn title_of_file(app: &App, v: &Viewer, file: i64) -> ApiResult<i64> {
    app.db
        .with(|c| {
            c.query_row(
                &format!(
                    "SELECT t.id FROM files f JOIN titles t ON t.id = f.title_id
                     WHERE f.id = ? AND f.missing = 0 AND {}",
                    visible(v.profile.kids)
                ),
                [file],
                |r| r.get(0),
            )
            .optional()
        })?
        .ok_or_else(ApiError::not_found)
}

fn no_room() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "no_room", "Комната закрыта - создайте новую со страницы фильма")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBody {
    file_id: Option<i64>,
    /// Или видео по ссылке (фильм из Обзора).
    link_id: Option<i64>,
    #[serde(default)]
    position: f64,
}

pub async fn create(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<CreateBody>) -> ApiResult<Json<Value>> {
    let (title, file) = match (b.file_id, b.link_id) {
        (Some(f), _) => (title_of_file(&app, &v, f)?, f),
        (None, Some(l)) => {
            let ok: bool =
                app.db.with(|c| c.query_row("SELECT EXISTS (SELECT 1 FROM links WHERE id = ?)", [l], |r| r.get(0)))?;
            if !ok {
                return Err(ApiError::not_found());
            }
            (0, 0)
        }
        _ => return Err(ApiError::bad("no_video", "Что смотреть вместе: файл или ссылка")),
    };
    let who = me(&app, &v)?;
    let name = who["name"].as_str().unwrap_or_default().to_string();
    let link = b.file_id.is_none().then_some(b.link_id).flatten();
    let id = app.rooms.create(v.profile.id, title, file, link, b.position.max(0.0), name);
    Ok(Json(json!({"id": id})))
}

/// Что за комната (без входа): фильм, кто там, где сейчас.
pub async fn info(State(app): State<Arc<App>>, _v: Viewer, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    app.rooms.with(&id, |r| Json(r.snapshot())).ok_or_else(no_room)
}

/// Войти: поток событий комнаты. Первое событие - снимок (место, люди, чат), дальше - изменения.
/// Закрыл вкладку - вышел.
pub async fn events(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<String>) -> ApiResult<impl IntoResponse> {
    let who = me(&app, &v)?;
    let (rx, snap) = app.rooms.join(&id, v.profile.id, who).ok_or_else(no_room)?;
    struct Leave {
        app: Arc<App>,
        id: String,
        profile: i64,
    }
    impl Drop for Leave {
        fn drop(&mut self) {
            self.app.rooms.leave(&self.id, self.profile);
        }
    }
    let guard = Leave { app: app.clone(), id, profile: v.profile.id };
    let first = Some(snap.to_string());
    let stream = futures_util::stream::unfold((rx, guard, first), |(mut rx, guard, first)| async move {
        if let Some(f) = first {
            return Some((Ok::<_, Infallible>(Event::default().data(f)), (rx, guard, None)));
        }
        match rx.recv().await {
            Ok(m) => Some((Ok(Event::default().data(m)), (rx, guard, None))),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                if let Some(snapshot) = guard.app.rooms.with(&guard.id, |room| room.snapshot()) {
                    return Some((Ok(Event::default().data(snapshot.to_string())), (rx, guard, None)));
                }
                None
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
        }
    });
    let sse = Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)));
    // Туннели и прокси не копят поток событий.
    Ok(([(header::CACHE_CONTROL, "no-cache"), (header::HeaderName::from_static("x-accel-buffering"), "no")], sse))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateBody {
    file_id: Option<i64>,
    playing: bool,
    position: f64,
}

/// Пауза, перемотка, следующая серия - у всех. Управляет любой в комнате.
pub async fn set_state(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Json(b): Json<StateBody>,
) -> ApiResult<Json<Value>> {
    let (cur_file, title, link) =
        app.rooms.with(&id, |r| (r.state.file_id, r.title_id, r.state.link_id)).ok_or_else(no_room)?;
    // У видео по ссылке файл не меняется; у сериала - только серии этого же сериала.
    let file = if link.is_some() { cur_file } else { b.file_id.unwrap_or(cur_file) };
    if file != cur_file && title_of_file(&app, &v, file)? != title {
        return Err(ApiError::bad("other_title", "В комнате - только серии этого сериала"));
    }
    if !b.position.is_finite() {
        return Err(ApiError::bad("bad_position", "Неверное место в видео"));
    }
    let who = me(&app, &v)?;
    let name = who["name"].as_str().unwrap_or_default().to_string();
    let st = app.rooms.set_state(&id, file, b.playing, b.position, name).ok_or_else(no_room)?;
    Ok(Json(json!(st)))
}

#[derive(Deserialize)]
pub struct ChatBody {
    body: String,
}

pub async fn chat(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Json(b): Json<ChatBody>,
) -> ApiResult<Json<Value>> {
    let body = super::social::text(&b.body, 500, "Сообщение")?;
    if body.is_empty() {
        return Err(ApiError::bad("empty", "Пустое сообщение"));
    }
    let key = format!("room:{}", v.profile.id);
    if app.message_limit.blocked(&key) {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Слишком часто - подождите минуту"));
    }
    app.message_limit.fail(&key);
    let who = me(&app, &v)?;
    app.rooms.say(&id, who, body).ok_or_else(no_room)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct ReactBody {
    emoji: String,
}

pub async fn react(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Json(b): Json<ReactBody>,
) -> ApiResult<Json<Value>> {
    if !REACTIONS.contains(&b.emoji.as_str()) {
        return Err(ApiError::bad("bad_emoji", "Такой реакции нет"));
    }
    let who = me(&app, &v)?;
    app.rooms.react(&id, who, &b.emoji).ok_or_else(no_room)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteBody {
    profile_id: i64,
}

/// Позвать друга: сообщение в переписку с фильмом и ссылкой на комнату.
pub async fn invite(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Json(b): Json<InviteBody>,
) -> ApiResult<Json<Value>> {
    let me_id = v.profile.id;
    let title = app.rooms.with(&id, |r| r.title_id).ok_or_else(no_room)?;
    let friend: bool = app.db.with(|c| {
        c.query_row(
            &format!(
                "SELECT EXISTS (SELECT 1 FROM friends f JOIN profiles p ON p.id = ?2
                   WHERE f.status = 'accepted' AND ((f.a = ?1 AND f.b = ?2) OR (f.a = ?2 AND f.b = ?1)) AND {SOCIAL})"
            ),
            params![me_id, b.profile_id],
            |r| r.get(0),
        )
    })?;
    if !friend || v.profile.kids {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "not_friends", "Звать можно только друзей"));
    }
    let link = app.rooms.with(&id, |r| r.state.link_id).flatten();
    let name: String = match link {
        Some(l) => app.db.with(|c| c.query_row("SELECT label FROM links WHERE id = ?", [l], |r| r.get(0)))?,
        None => app.db.with(|c| c.query_row("SELECT title FROM titles WHERE id = ?", [title], |r| r.get(0)))?,
    };
    app.db.with(|c| {
        c.execute(
            "INSERT INTO messages (from_id, to_id, body, title_id, created_at) VALUES (?, ?, ?, ?, ?)",
            params![
                me_id,
                b.profile_id,
                format!("Зову смотреть вместе «{name}»: /room/{id}"),
                (title > 0).then_some(title),
                now_ms()
            ],
        )
    })?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct VoiceBody {
    on: bool,
}

/// Войти в голосовой чат комнаты или выйти.
pub async fn voice(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Json(b): Json<VoiceBody>,
) -> ApiResult<Json<Value>> {
    app.rooms.voice(&id, v.profile.id, b.on).ok_or_else(no_room)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct SignalBody {
    to: i64,
    data: Value,
}

/// Сигнал WebRTC другому человеку в комнате (сам голос идёт между браузерами, не через нас).
pub async fn signal(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Json(b): Json<SignalBody>,
) -> ApiResult<Json<Value>> {
    if b.data.to_string().len() > 20_000 {
        return Err(ApiError::bad("too_big", "Слишком большой сигнал"));
    }
    app.rooms.signal(&id, v.profile.id, b.to, b.data).ok_or_else(no_room)?;
    Ok(Json(json!({"ok": true})))
}
