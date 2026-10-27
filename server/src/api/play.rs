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
use std::time::Duration;

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

/// Файл, который этот профиль может смотреть (скрытое и «взрослое» в детском профиле - нет).
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
    /// Отметил хост (иначе - из глав файла).
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
    // С какого места продолжить (досмотренное - сначала).
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
    type EpisodePreview = (Option<i32>, Option<i32>, Option<String>, Option<String>);
    let next_file: Option<EpisodePreview> = match next {
        Some(n) => app
            .db
            .with(|c| {
                c.query_row("SELECT season, episode, name, still FROM files WHERE id = ?", [n], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })
            })
            .ok(),
        None => None,
    };
    let next_still = next_file.map(|(season, episode, name, still)| json!({
        "label": super::catalog::episode_label(season, episode, crate::episode_locale::localized(&app, name.as_deref()).as_deref()),
        "still": super::img(still),
    }));
    // Сколько Мбит/с нужно для «Оригинала»: плеер сравнивает с замером скорости зрителя.
    let bitrate = f.probe.as_ref().and_then(|p| p.duration).filter(|d| *d > 0.0).and_then(|d| {
        let size = std::fs::metadata(&f.path).ok()?.len() as f64;
        Some((size * 8.0 / d / 1e6 * 10.0).round() / 10.0)
    });
    Ok(Json(json!({
        "play": info, "titleId": f.title_id, "kind": f.kind, "title": f.title, "bitrate": bitrate,
        "season": f.season,
        "episode": super::catalog::episode_label(f.season, f.episode, crate::episode_locale::localized(&app, f.name.as_deref()).as_deref()),
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

#[derive(Deserialize)]
pub struct PackQuery {
    season: Option<i32>,
}

/// Канал в поток ответа: архив пишется в отдельном потоке, кусочками по 1 МБ.
struct ChanWriter {
    tx: tokio::sync::mpsc::Sender<std::io::Result<axum::body::Bytes>>,
    buf: Vec<u8>,
}

impl std::io::Write for ChanWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(b);
        if self.buf.len() >= 1 << 20 {
            self.flush()?;
        }
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let chunk = axum::body::Bytes::from(std::mem::take(&mut self.buf));
        // Скачивание прервали - прекращаем читать файлы.
        self.tx.blocking_send(Ok(chunk)).map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
}

/// Скачать фильм (все части) или сериал (весь или сезон) одним архивом ZIP - потоком, без
/// временных файлов. Внутри - «Сезон N/имя файла».
pub async fn download_all(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Query(q): Query<PackQuery>,
) -> ApiResult<Response> {
    if !(app.db.setting_or("site.downloads", "1") == "1" && v.can_download) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "no_downloads", "Хост не разрешил скачивание"));
    }
    let card =
        super::cards(&app, v.profile.kids, "t.id = ?", "t.id", 1, &[&id])?.pop().ok_or_else(ApiError::not_found)?;
    let rows: Vec<(String, Option<i32>)> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT path, season FROM files WHERE title_id = ?1 AND missing = 0 AND (?2 IS NULL OR season = ?2)
             ORDER BY season, episode, path",
        )?;
        let r = st.query_map(params![id, q.season], |r| Ok((r.get(0)?, r.get(1)?)))?;
        r.collect()
    })?;
    let series = card.kind == "series";
    let mut entries = Vec::new();
    for (path, season) in rows {
        let p = PathBuf::from(&path);
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        let file = p.file_name().and_then(|n| n.to_str()).unwrap_or("video").to_string();
        let name = match (series, season) {
            (true, Some(s)) => format!("Сезон {s}/{file}"),
            _ => file,
        };
        entries.push(crate::zipstream::Entry { name, path: p, size: meta.len() });
    }
    if entries.is_empty() {
        return Err(ApiError::not_found());
    }
    let total = crate::zipstream::total(&entries);
    let clean: String = card.title.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c }).collect();
    let name = match q.season {
        Some(s) => format!("{clean} - {s} сезон.zip"),
        None => format!("{clean}.zip"),
    };
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tokio::task::spawn_blocking(move || {
        let mut w = ChanWriter { tx: tx.clone(), buf: Vec::with_capacity(1 << 20) };
        if let Err(e) = crate::zipstream::write(&entries, &mut w) {
            let _ = tx.blocking_send(Err(e));
        }
    });
    let chunks = futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|c| (c, rx)) });
    let body = axum::body::Body::from_stream(chunks);
    Response::builder()
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_LENGTH, total)
        .header(header::CONTENT_DISPOSITION, crate::util::content_disposition("attachment", &name))
        .body(body)
        .map_err(ApiError::internal)
}

fn no_ffmpeg() -> ApiError {
    ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        "no_ffmpeg",
        "Этот формат браузер не играет, а на сервере Nova нет ffmpeg",
    )
}

pub fn busy() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "busy",
        "Сервер Nova сейчас перекодирует другие видео - попробуйте через минуту",
    )
}

#[derive(Deserialize)]
pub struct RemuxQuery {
    #[serde(default)]
    t: f64,
    #[serde(default)]
    audio: usize,
    /// Браузер умеет HEVC (Safari) - видео не перекодируем.
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

// ---------- готовая копия без перекодирования (HLS из fMP4) ----------

#[derive(Deserialize)]
pub struct CopyQuery {
    /// С какого места начнут смотреть: ответ - когда оно уже готово.
    #[serde(default)]
    from: f64,
}

/// Видео копируется как есть: H.264 (8 бит) или HEVC; остальное - только перекодированием.
fn copy_source(app: &App, v: &Viewer, id: i64, audio: usize) -> ApiResult<(FileRow, crate::copy::Key, bool, bool)> {
    let f = file_row(app, v, id)?;
    let p =
        f.probe.as_ref().ok_or_else(|| ApiError::bad("not_ready", "Файл ещё разбирается - попробуйте через минуту"))?;
    let vid = p.video.as_ref().ok_or_else(|| ApiError::bad("no_video", "В файле нет видео"))?;
    let hevc = vid.codec == "hevc";
    if !(hevc || vid.codec == "h264" && !vid.pix_fmt.contains("10")) {
        return Err(ApiError::bad("transcode", "Этот файл нужно перекодировать - включите адаптивное качество"));
    }
    let audio = audio.min(p.audio.len().saturating_sub(1));
    let copy_audio = p.audio.get(audio).is_some_and(|a| a.codec == "aac");
    Ok((f, crate::copy::Key { file: id, audio }, hevc, copy_audio))
}

pub async fn copy_playlist(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path((id, audio)): Path<(i64, usize)>,
    Query(q): Query<CopyQuery>,
) -> ApiResult<Response> {
    let ffmpeg = app.tools.read().ffmpeg.clone().ok_or_else(no_ffmpeg)?;
    let (f, key, hevc, copy_audio) = copy_source(&app, &v, id, audio)?;
    let src = crate::copy::Source { input: &f.path, hevc, copy_audio };
    let from = q.from.clamp(0.0, 60.0 * 60.0 * 24.0);
    let text = app.copies.playlist(&app, &ffmpeg, key, src, from).await?;
    Ok(m3u8(text))
}

pub async fn copy_part(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path((id, audio, name)): Path<(i64, usize, String)>,
) -> ApiResult<Response> {
    let (_, key, _, _) = copy_source(&app, &v, id, audio)?;
    let path = app.copies.part(&app, key, &name)?;
    let bytes = tokio::fs::read(&path).await?;
    Ok(([(header::CONTENT_TYPE, "video/mp4"), (header::CACHE_CONTROL, "private, max-age=3600")], bytes).into_response())
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
        f.probe.as_ref().ok_or_else(|| ApiError::bad("not_ready", "Файл ещё разбирается - попробуйте через минуту"))?;
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
    let key = (id, sid.clone());
    if let Some(vtt) = SUBS.lock().get(&key).cloned() {
        return Ok(vtt_response(vtt));
    }
    // Встроенные субтитры ffmpeg достаёт, читая весь файл (гигабайты): не больше двух сразу и
    // не дольше 2 минут, готовое — в памяти. Иначе десяток запросов зрителя занимал диск хоста.
    let _permit = tokio::time::timeout(Duration::from_secs(30), SUBS_SLOTS.acquire())
        .await
        .map_err(|_| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "busy", "Сервер занят - попробуйте ещё раз"))?
        .map_err(ApiError::internal)?;
    if let Some(vtt) = SUBS.lock().get(&key).cloned() {
        return Ok(vtt_response(vtt));
    }
    let ffmpeg = app.tools.read().ffmpeg.clone();
    let vtt: Arc<str> =
        tokio::time::timeout(Duration::from_secs(120), stream::subtitle(&f.path, &sid, ffmpeg.as_deref()))
            .await
            .map_err(|_| ApiError::bad("subs", "Субтитры не успели прочитаться"))?
            .map_err(|e| ApiError::bad("subs", e.to_string()))?
            .into();
    let mut cache = SUBS.lock();
    if cache.len() >= 64 {
        cache.clear();
    }
    cache.insert(key, vtt.clone());
    Ok(vtt_response(vtt))
}

static SUBS_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
type SubsCache = parking_lot::Mutex<std::collections::HashMap<(i64, String), Arc<str>>>;
static SUBS: std::sync::LazyLock<SubsCache> = std::sync::LazyLock::new(Default::default);

fn vtt_response(vtt: Arc<str>) -> Response {
    (
        [(header::CONTENT_TYPE, "text/vtt; charset=utf-8"), (header::CACHE_CONTROL, "private, max-age=3600")],
        vtt.to_string(),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct ProgressBody {
    position: f64,
    duration: f64,
    /// Как идёт просмотр у зрителя: замер скорости, качество, остановки - для «Управления».
    #[serde(default)]
    net: Option<NetBody>,
}

#[derive(Deserialize)]
pub struct NetBody {
    mbit: Option<f64>,
    quality: Option<String>,
    stalls: Option<u32>,
}

/// Последнее, что плеер зрителя сообщил о сети (в памяти: после перезапуска не нужно).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayStat {
    pub mbit: Option<f64>,
    pub quality: String,
    pub stalls: u32,
    pub at: i64,
}

/// Качество - только из известных: «orig», «auto» или высота вроде «720p».
fn quality_ok(q: &str) -> bool {
    matches!(q, "orig" | "auto")
        || q.strip_suffix('p').is_some_and(|n| (3..=4).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()))
}

impl NetBody {
    fn stat(&self) -> Option<PlayStat> {
        let quality = self.quality.as_deref().filter(|q| quality_ok(q))?.to_string();
        Some(PlayStat {
            mbit: self
                .mbit
                .filter(|m| m.is_finite() && (0.0..100_000.0).contains(m))
                .map(|m| (m * 10.0).round() / 10.0),
            quality,
            stalls: self.stalls.unwrap_or(0).min(100_000),
            at: crate::util::now_ms(),
        })
    }
}

/// Сколько отдаём на замер скорости: по умолчанию 1,5 МБ, не больше 4 МБ.
const SPEED_DEFAULT: usize = 1_500_000;
const SPEED_MAX: usize = 4_000_000;

#[derive(Deserialize)]
pub struct SpeedQuery {
    bytes: Option<usize>,
}

/// Замер скорости до хоста: плеер качает несжимаемый кусок и сам считает Мбит/с. Только для
/// вошедших и не чаще 20 раз в минуту на профиль - иначе это был бы бесплатный генератор трафика.
pub async fn speedtest(State(app): State<Arc<App>>, v: Viewer, Query(q): Query<SpeedQuery>) -> ApiResult<Response> {
    let key = format!("speed:{}", v.profile.id);
    if app.speed_limit.blocked(&key) {
        return Err(ApiError::bad("rate_limited", "Слишком часто - подождите минуту"));
    }
    app.speed_limit.fail(&key);
    let n = q.bytes.unwrap_or(SPEED_DEFAULT).clamp(1, SPEED_MAX);
    let mut body = vec![0u8; n];
    rand::fill(&mut body[..]);
    Ok(([(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-store")], body)
        .into_response())
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
    if let Some(stat) = b.net.as_ref().and_then(NetBody::stat) {
        let mut all = app.playback.lock();
        // Старое (больше 10 минут) - убираем, чтобы карта не росла.
        all.retain(|_, s| stat.at - s.at < 600_000);
        all.insert(v.profile.id, stat);
    }
    let watched = done && super::catalog::mark_if_finished(&app, v.profile.id, f.title_id)?;
    Ok(Json(json!({"ok": true, "done": done, "watched": watched})))
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

/// Хост отмечает заставку и титры прямо в плеере - на весь сезон.
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

/// Где этот зритель остановился в файле - для другого устройства (открытый там плеер сверяется,
/// не досмотрели ли дальше здесь).
pub async fn get_progress(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    file_row(&app, &v, id)?;
    let row: Option<(f64, f64, i64)> = app.db.with(|c| {
        c.query_row(
            "SELECT position, duration, updated_at FROM progress WHERE profile_id = ? AND file_id = ?",
            params![v.profile.id, id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
    })?;
    Ok(Json(match row {
        Some((position, duration, at)) => json!({"position": position, "duration": duration, "updatedAt": at}),
        None => json!({"position": null, "duration": null, "updatedAt": null}),
    }))
}

#[cfg(test)]
mod net_tests {
    use super::*;

    #[test]
    fn only_known_qualities() {
        for q in ["orig", "auto", "720p", "1080p"] {
            assert!(quality_ok(q), "{q}");
        }
        for q in ["", "p", "72p", "10800p", "720", "<b>", "auto "] {
            assert!(!quality_ok(q), "{q}");
        }
        let n = NetBody { mbit: Some(f64::NAN), quality: Some("720p".into()), stalls: Some(3) };
        let s = n.stat().unwrap();
        assert_eq!((s.mbit, s.quality.as_str(), s.stalls), (None, "720p", 3));
        assert!(NetBody { mbit: Some(5.0), quality: Some("evil".into()), stalls: None }.stat().is_none());
    }
}
