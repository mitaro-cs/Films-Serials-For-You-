//! Просмотр: сведения о дорожках, поток файла, перепаковка ffmpeg, субтитры, прогресс.

use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::library::probe::Probe;
use crate::stream::{self, RemuxOpts};
use crate::util::now_ms;
use crate::App;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

struct FileRow {
    path: PathBuf,
    probe: Option<Probe>,
    title: String,
    season: Option<i32>,
    episode: Option<i32>,
}

fn file_row(app: &App, id: i64) -> ApiResult<FileRow> {
    let (path, probe, title, season, episode, hidden): (String, Option<String>, String, Option<i32>, Option<i32>, bool) = app.db.with(|c| {
        c.query_row(
            "SELECT f.path, f.probe, t.title, f.season, f.episode, t.hidden FROM files f JOIN titles t ON t.id = f.title_id
             WHERE f.id = ? AND f.missing = 0",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
    })?;
    if hidden {
        return Err(ApiError::not_found());
    }
    Ok(FileRow {
        path: PathBuf::from(path),
        probe: probe.and_then(|p| serde_json::from_str(&p).ok()),
        title,
        season,
        episode,
    })
}

pub async fn info(State(app): State<Arc<App>>, _v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let f = file_row(&app, id)?;
    let has_ff = app.tools.read().ffmpeg.is_some();
    let path = f.path.clone();
    let probe = f.probe.clone();
    let info = tokio::task::spawn_blocking(move || stream::play_info(&path, probe.as_ref(), has_ff))
        .await
        .map_err(ApiError::internal)?;
    let (title_id, kind): (i64, String) = app.db.with(|c| {
        c.query_row("SELECT t.id, t.kind FROM files f JOIN titles t ON t.id = f.title_id WHERE f.id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
    })?;
    // Соседние серии для «Следующая серия».
    let (prev, next): (Option<i64>, Option<i64>) = app.db.with(|c| {
        let mut st =
            c.prepare("SELECT id FROM files WHERE title_id = ? AND missing = 0 ORDER BY season, episode, path")?;
        let ids: Vec<i64> = st.query_map([title_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let i = ids.iter().position(|x| *x == id);
        Ok((
            i.and_then(|i| i.checked_sub(1)).and_then(|i| ids.get(i).copied()),
            i.and_then(|i| ids.get(i + 1).copied()),
        ))
    })?;
    let name: Option<String> =
        app.db.with(|c| c.query_row("SELECT name FROM files WHERE id = ?", [id], |r| r.get(0)))?;
    Ok(Json(json!({
        "play": info, "titleId": title_id, "kind": kind, "title": f.title,
        "episode": super::catalog::episode_label(f.season, f.episode, name.as_deref()),
        "prev": prev, "next": next,
    })))
}

pub async fn raw(
    State(app): State<Arc<App>>,
    _v: Viewer,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let f = file_row(&app, id)?;
    Ok(stream::file(&f.path, &headers, None).await?)
}

pub async fn download(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    if !(app.db.setting_or("site.downloads", "1") == "1" && v.can_download) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "no_downloads", "Хост не разрешил скачивание"));
    }
    let f = file_row(&app, id)?;
    let name = f.path.file_name().and_then(|n| n.to_str()).unwrap_or("video").to_string();
    Ok(stream::file(&f.path, &headers, Some(&name)).await?)
}

#[derive(Deserialize)]
pub struct RemuxQuery {
    #[serde(default)]
    t: f64,
    #[serde(default)]
    audio: usize,
    /// Браузер умеет HEVC (Safari) — видео не перекодируем.
    #[serde(default)]
    hevc: u8,
    /// Своё ограничение высоты при перекодировании (медленный интернет).
    q: Option<i64>,
}

pub async fn remux(
    State(app): State<Arc<App>>,
    _v: Viewer,
    Path(id): Path<i64>,
    Query(q): Query<RemuxQuery>,
) -> ApiResult<Response> {
    let f = file_row(&app, id)?;
    let ffmpeg = app.tools.read().ffmpeg.clone().ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "no_ffmpeg",
            "Этот формат браузер не играет, а ffmpeg у хоста не установлен",
        )
    })?;
    let p = f.probe.unwrap_or_default();
    let vcodec = p.video.as_ref().map(|v| v.codec.as_str()).unwrap_or("");
    let ten_bit = p.video.as_ref().is_some_and(|v| v.pix_fmt.contains("10"));
    let copy_video =
        (vcodec == "h264" && !ten_bit) || (vcodec == "hevc" && q.hevc == 1) || vcodec.is_empty() && p.video.is_none();
    let acodec = p.audio.get(q.audio).map(|a| a.codec.as_str()).unwrap_or("");
    let opts = RemuxOpts {
        start: q.t.clamp(0.0, 60.0 * 60.0 * 24.0),
        audio: q.audio.min(p.audio.len().saturating_sub(1)),
        copy_video,
        hevc: vcodec == "hevc",
        copy_audio: matches!(acodec, "aac" | "mp3"),
        max_height: q.q.unwrap_or(1080).clamp(240, 2160),
    };
    let sem = if copy_video { app.remux.clone() } else { app.transcode.clone() };
    let permit = sem.try_acquire_owned().map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "busy",
            "Компьютер хоста сейчас перекодирует другие видео — попробуйте через минуту",
        )
    })?;
    Ok(stream::remux(&ffmpeg, stream::remux_args(&f.path, &opts), permit)?)
}

pub async fn subs(
    State(app): State<Arc<App>>,
    _v: Viewer,
    Path((id, sid)): Path<(i64, String)>,
) -> ApiResult<Response> {
    if !sid.chars().next().is_some_and(|c| c == 'f' || c == 'e') || sid.len() > 4 {
        return Err(ApiError::not_found());
    }
    let f = file_row(&app, id)?;
    let ffmpeg = app.tools.read().ffmpeg.clone();
    let vtt =
        stream::subtitle(&f.path, &sid, ffmpeg.as_deref()).await.map_err(|e| ApiError::bad("subs", e.to_string()))?;
    Ok(([(header::CONTENT_TYPE, "text/vtt; charset=utf-8"), (header::CACHE_CONTROL, "private, max-age=3600")], vtt)
        .into_response())
}

#[derive(Deserialize)]
pub struct ProgressBody {
    position: f64,
    duration: f64,
}

pub async fn save_progress(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<ProgressBody>,
) -> ApiResult<Json<Value>> {
    if !(b.position.is_finite() && b.duration.is_finite()) || b.position < 0.0 || b.duration <= 0.0 {
        return Err(ApiError::bad("bad_progress", "Неверное время"));
    }
    let pos = b.position.min(b.duration);
    // Досмотрено: осталось меньше 7 % (титры).
    let done = pos >= b.duration * 0.93;
    app.db.with(|c| {
        c.execute(
            "INSERT INTO progress (user_id, file_id, position, duration, done, updated_at)
             SELECT ?, id, ?, ?, ?, ? FROM files WHERE id = ?
             ON CONFLICT(user_id, file_id) DO UPDATE SET position = excluded.position, duration = excluded.duration,
               done = excluded.done, updated_at = excluded.updated_at",
            params![v.id, pos, b.duration, done, now_ms(), id],
        )
    })?;
    Ok(Json(json!({"ok": true, "done": done})))
}

pub async fn clear_progress(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    // Убрать из «Продолжить»: весь прогресс этого произведения.
    app.db.with(|c| {
        c.execute(
            "DELETE FROM progress WHERE user_id = ? AND file_id IN (SELECT id FROM files WHERE title_id = (SELECT title_id FROM files WHERE id = ?))",
            params![v.id, id],
        )
    })?;
    Ok(Json(json!({"ok": true})))
}
