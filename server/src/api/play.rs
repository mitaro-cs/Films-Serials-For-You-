//! Просмотр: сведения о дорожках, поток файла, перепаковка ffmpeg, адаптивный HLS, субтитры,
//! заставка и титры, кадры для полосы перемотки, прогресс профиля.

use super::visible;
use crate::auth::{Manager, Viewer};
use crate::error::{ApiError, ApiResult};
use crate::library::probe::{chapter_markers, Probe};
use crate::stream::{self, RemuxOpts};
use crate::util::tick_ms;
use crate::App;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

pub struct FileRow {
    pub path: PathBuf,
    pub probe: Option<Probe>,
    pub title: String,
    pub title_id: i64,
    pub kind: String,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub name: Option<String>,
    pub trickplay: Option<String>,
}

/// Файл, который этот профиль может смотреть (скрытое и «взрослое» в детском профиле — нет).
pub fn file_row(app: &App, v: &Viewer, id: i64) -> ApiResult<FileRow> {
    let sql = format!(
        "SELECT f.path, f.probe, t.title, t.id, t.kind, f.season, f.episode, f.name, f.trickplay
         FROM files f JOIN titles t ON t.id = f.title_id WHERE f.id = ? AND f.missing = 0 AND {}",
        visible(v.profile.kids)
    );
    let row = app.db.with(|c| {
        c.query_row(&sql, [id], |r| {
            Ok(FileRow {
                path: PathBuf::from(r.get::<_, String>(0)?),
                probe: r.get::<_, Option<String>>(1)?.and_then(|p| serde_json::from_str(&p).ok()),
                title: r.get(2)?,
                title_id: r.get(3)?,
                kind: r.get(4)?,
                season: r.get(5)?,
                episode: r.get(6)?,
                name: r.get(7)?,
                trickplay: r.get(8)?,
            })
        })
    })?;
    Ok(row)
}

#[derive(Debug, Default, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Markers {
    pub intro: Option<(f64, f64)>,
    pub credits: Option<f64>,
    /// Отметил хост (иначе — из глав файла).
    pub manual: bool,
}

/// Заставка и титры: отметки хоста на сезон важнее глав файла.
pub fn markers_for(app: &App, f: &FileRow) -> Markers {
    let season = f.season.unwrap_or(0);
    let manual: Option<(Option<f64>, Option<f64>, Option<f64>)> = app
        .db
        .with(|c| {
            c.query_row(
                "SELECT intro_start, intro_end, credits FROM markers WHERE title_id = ? AND season = ?",
                params![f.title_id, season],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
        })
        .ok()
        .flatten();
    let (ch_intro, ch_credits) = f.probe.as_ref().map(|p| chapter_markers(&p.chapters, p.duration)).unwrap_or_default();
    match manual {
        Some((s, e, cr)) => Markers {
            intro: match (s, e) {
                (Some(s), Some(e)) if e > s => Some((s, e)),
                _ => ch_intro,
            },
            credits: cr.or(ch_credits),
            manual: true,
        },
        None => Markers { intro: ch_intro, credits: ch_credits, manual: false },
    }
}

pub async fn info(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let f = file_row(&app, &v, id)?;
    let has_ff = app.tools.read().ffmpeg.is_some();
    let path = f.path.clone();
    let probe = f.probe.clone();
    let info = tokio::task::spawn_blocking(move || stream::play_info(&path, probe.as_ref(), has_ff))
        .await
        .map_err(ApiError::internal)?;
    // Соседние серии для «Следующая серия».
    let (prev, next): (Option<i64>, Option<i64>) = app.db.with(|c| {
        let mut st =
            c.prepare("SELECT id FROM files WHERE title_id = ? AND missing = 0 ORDER BY season, episode, path")?;
        let ids: Vec<i64> = st.query_map([f.title_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let i = ids.iter().position(|x| *x == id);
        Ok((
            i.and_then(|i| i.checked_sub(1)).and_then(|i| ids.get(i).copied()),
            i.and_then(|i| ids.get(i + 1).copied()),
        ))
    })?;
    // С какого места продолжить (досмотренное — сначала).
    let position: f64 = app
        .db
        .with(|c| {
            c.query_row(
                "SELECT CASE WHEN done THEN 0 ELSE position END FROM progress WHERE profile_id = ? AND file_id = ?",
                params![v.profile.id, id],
                |r| r.get(0),
            )
            .optional()
        })?
        .unwrap_or(0.0);
    let trickplay: Option<Value> = f
        .trickplay
        .as_deref()
        .and_then(|t| serde_json::from_str(t).ok())
        .filter(|t: &Value| t["count"].as_i64().unwrap_or(0) > 0);
    let next_still: Option<Value> = match next {
        Some(n) => app
            .db
            .with(|c| {
                c.query_row("SELECT season, episode, name, still FROM files WHERE id = ?", [n], |r| {
                    Ok(json!({
                        "label": super::catalog::episode_label(r.get(0)?, r.get(1)?, r.get::<_, Option<String>>(2)?.as_deref()),
                        "still": super::img(r.get(3)?),
                    }))
                })
            })
            .ok(),
        None => None,
    };
    Ok(Json(json!({
        "play": info, "titleId": f.title_id, "kind": f.kind, "title": f.title,
        "season": f.season,
        "episode": super::catalog::episode_label(f.season, f.episode, f.name.as_deref()),
        "prev": prev, "next": next, "nextInfo": next_still, "position": position,
        "markers": markers_for(&app, &f), "trickplay": trickplay,
        "hls": has_ff && f.probe.as_ref().is_some_and(|p| p.video.is_some() && p.duration.is_some()),
    })))
}

pub async fn raw(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let f = file_row(&app, &v, id)?;
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
    let f = file_row(&app, &v, id)?;
    let name = f.path.file_name().and_then(|n| n.to_str()).unwrap_or("video").to_string();
    Ok(stream::file(&f.path, &headers, Some(&name)).await?)
}

fn no_ffmpeg() -> ApiError {
    ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        "no_ffmpeg",
        "Этот формат браузер не играет, а ffmpeg у хоста не установлен",
    )
}

pub fn busy() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "busy",
        "Компьютер хоста сейчас перекодирует другие видео — попробуйте через минуту",
    )
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
    v: Viewer,
    Path(id): Path<i64>,
    Query(q): Query<RemuxQuery>,
) -> ApiResult<Response> {
    let f = file_row(&app, &v, id)?;
    let ffmpeg = app.tools.read().ffmpeg.clone().ok_or_else(no_ffmpeg)?;
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
    let permit = sem.try_acquire_owned().map_err(|_| busy())?;
    Ok(stream::remux(&ffmpeg, stream::remux_args(&f.path, &opts), permit)?)
}

// ---------- адаптивный поток HLS ----------

#[derive(Deserialize)]
pub struct AudioQuery {
    #[serde(default)]
    audio: usize,
}

fn m3u8(body: String) -> Response {
    ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl"), (header::CACHE_CONTROL, "no-cache")], body)
        .into_response()
}

fn hls_source(app: &App, v: &Viewer, id: i64) -> ApiResult<(FileRow, f64, i64)> {
    if app.tools.read().ffmpeg.is_none() {
        return Err(no_ffmpeg());
    }
    let f = file_row(app, v, id)?;
    let p =
        f.probe.as_ref().ok_or_else(|| ApiError::bad("not_ready", "Файл ещё разбирается — попробуйте через минуту"))?;
    let duration =
        p.duration.filter(|d| *d > 0.0).ok_or_else(|| ApiError::bad("not_ready", "Неизвестна длительность"))?;
    let height = p
        .video
        .as_ref()
        .map(|v| v.height)
        .filter(|h| *h > 0)
        .ok_or_else(|| ApiError::bad("no_video", "В файле нет видео"))?;
    Ok((f, duration, height))
}

pub async fn hls_master(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Query(q): Query<AudioQuery>,
) -> ApiResult<Response> {
    let (_, _, height) = hls_source(&app, &v, id)?;
    Ok(m3u8(crate::hls::master(height, q.audio)))
}

pub async fn hls_variant(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path((id, h)): Path<(i64, i64)>,
    Query(q): Query<AudioQuery>,
) -> ApiResult<Response> {
    let (_, duration, height) = hls_source(&app, &v, id)?;
    if !crate::hls::levels(height).contains(&h) {
        return Err(ApiError::not_found());
    }
    Ok(m3u8(crate::hls::variant(duration, q.audio)))
}

pub async fn hls_segment(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path((id, h, seg)): Path<(i64, i64, String)>,
    Query(q): Query<AudioQuery>,
) -> ApiResult<Response> {
    let n: u64 = seg.strip_suffix(".ts").and_then(|n| n.parse().ok()).ok_or_else(ApiError::not_found)?;
    let (f, duration, height) = hls_source(&app, &v, id)?;
    if !crate::hls::levels(height).contains(&h) || n >= crate::hls::segments(duration) {
        return Err(ApiError::not_found());
    }
    let audio = q.audio.min(f.probe.as_ref().map(|p| p.audio.len().saturating_sub(1)).unwrap_or(0));
    let ffmpeg = app.tools.read().ffmpeg.clone().ok_or_else(no_ffmpeg)?;
    let path = crate::hls::Job { file: id, height: h, audio }.segment(&app, &ffmpeg, &f.path, height, n).await?;
    let bytes = tokio::fs::read(&path).await?;
    Ok(([(header::CONTENT_TYPE, "video/mp2t"), (header::CACHE_CONTROL, "private, max-age=600")], bytes).into_response())
}

// ---------- кадры для полосы перемотки ----------

pub async fn trickplay(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path((id, sheet)): Path<(i64, String)>,
) -> ApiResult<Response> {
    let n: u32 = sheet.strip_suffix(".jpg").and_then(|n| n.parse().ok()).ok_or_else(ApiError::not_found)?;
    file_row(&app, &v, id)?;
    let path = app.data.join("trickplay").join(id.to_string()).join(format!("{n}.jpg"));
    let bytes = tokio::fs::read(&path).await?;
    Ok(([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=86400")], bytes)
        .into_response())
}

pub async fn subs(State(app): State<Arc<App>>, v: Viewer, Path((id, sid)): Path<(i64, String)>) -> ApiResult<Response> {
    if !sid.chars().next().is_some_and(|c| c == 'f' || c == 'e') || sid.len() > 4 {
        return Err(ApiError::not_found());
    }
    let f = file_row(&app, &v, id)?;
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
    let f = file_row(&app, &v, id)?;
    let pos = b.position.min(b.duration);
    // Досмотрено: начались титры или осталось меньше 7 %.
    let credits = markers_for(&app, &f).credits;
    let done = pos >= b.duration * 0.93 || credits.is_some_and(|c| pos >= c);
    app.db.with(|c| {
        c.execute(
            "INSERT INTO progress (profile_id, file_id, position, duration, done, updated_at) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(profile_id, file_id) DO UPDATE SET position = excluded.position, duration = excluded.duration,
               done = excluded.done, updated_at = excluded.updated_at",
            params![v.profile.id, id, pos, b.duration, done, tick_ms()],
        )
    })?;
    Ok(Json(json!({"ok": true, "done": done})))
}

pub async fn clear_progress(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    // Убрать из «Продолжить»: весь прогресс этого произведения.
    app.db.with(|c| {
        c.execute(
            "DELETE FROM progress WHERE profile_id = ? AND file_id IN
               (SELECT id FROM files WHERE title_id = (SELECT title_id FROM files WHERE id = ?))",
            params![v.profile.id, id],
        )
    })?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkersBody {
    intro_start: Option<f64>,
    intro_end: Option<f64>,
    credits: Option<f64>,
}

/// Хост отмечает заставку и титры прямо в плеере — на весь сезон.
pub async fn set_markers(
    State(app): State<Arc<App>>,
    m: Manager,
    Path(id): Path<i64>,
    Json(b): Json<MarkersBody>,
) -> ApiResult<Json<Value>> {
    let f = file_row(&app, &m.0, id)?;
    let ok = |x: Option<f64>| x.is_none_or(|x| x.is_finite() && x >= 0.0);
    if !(ok(b.intro_start) && ok(b.intro_end) && ok(b.credits)) {
        return Err(ApiError::bad("bad_value", "Неверное время"));
    }
    if let (Some(s), Some(e)) = (b.intro_start, b.intro_end) {
        if e <= s || e - s > 600.0 {
            return Err(ApiError::bad("bad_value", "Конец заставки должен быть позже начала (до 10 минут)"));
        }
    }
    app.db.with(|c| {
        if b.intro_start.is_none() && b.intro_end.is_none() && b.credits.is_none() {
            c.execute(
                "DELETE FROM markers WHERE title_id = ? AND season = ?",
                params![f.title_id, f.season.unwrap_or(0)],
            )
        } else {
            c.execute(
                "INSERT INTO markers (title_id, season, intro_start, intro_end, credits) VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(title_id, season) DO UPDATE SET intro_start = excluded.intro_start,
                   intro_end = excluded.intro_end, credits = excluded.credits",
                params![f.title_id, f.season.unwrap_or(0), b.intro_start, b.intro_end, b.credits],
            )
        }
    })?;
    Ok(Json(json!({"ok": true, "markers": markers_for(&app, &f)})))
}
