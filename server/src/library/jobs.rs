//! Фоновые задачи библиотеки: обход папок и разбор файлов через ffprobe.

use super::probe;
use super::scan::{scan, ScanStats};
use crate::App;
use rusqlite::{params, OptionalExtension};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ScanState {
    pub running: bool,
    pub last: Option<ScanStats>,
    pub at: Option<i64>,
    pub error: Option<String>,
}

/// Обход раз в 10 минут и по просьбе (новая папка, кнопка «Обновить»).
pub async fn scan_worker(app: Arc<App>) {
    loop {
        app.scan_state.lock().running = true;
        let a = app.clone();
        let res = tokio::task::spawn_blocking(move || scan(&a.db, &a.images)).await;
        {
            let mut st = app.scan_state.lock();
            st.running = false;
            st.at = Some(crate::util::now_ms());
            match res {
                Ok(Ok(stats)) => {
                    tracing::info!(
                        files = stats.files,
                        added = stats.added,
                        missing = stats.missing,
                        "библиотека обновлена"
                    );
                    st.error = None;
                    st.last = Some(stats);
                }
                Ok(Err(e)) => st.error = Some(e.to_string()),
                Err(e) => st.error = Some(e.to_string()),
            }
        }
        app.meta_kick.notify_one();
        app.kick_probe();
        let _ = tokio::time::timeout(Duration::from_secs(600), app.scan_kick.notified()).await;
    }
}

/// ffprobe для новых файлов и кадр-превью, если картинки нет.
pub async fn probe_worker(app: Arc<App>) {
    loop {
        let tools = app.tools.read().clone();
        if let Some(ffprobe) = tools.ffprobe {
            let batch: Vec<(i64, String, Option<String>)> = app
                .db
                .with(|c| {
                    let mut st =
                        c.prepare("SELECT id, path, still FROM files WHERE probe IS NULL AND missing = 0 LIMIT 50")?;
                    let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                    r.collect()
                })
                .unwrap_or_default();
            if !batch.is_empty() {
                for (id, path, still) in batch {
                    let path = PathBuf::from(path);
                    let p = probe::probe(&ffprobe, &path).await.unwrap_or_default();
                    let mut frame = None;
                    if still.is_none() && p.video.is_some() {
                        if let Some(ffmpeg) = &tools.ffmpeg {
                            let at = p.duration.map(|d| (d * 0.15).min(600.0)).unwrap_or(30.0);
                            if let Ok(jpg) = probe::frame(ffmpeg, &path, at, 640).await {
                                frame = app.images.save(&jpg).ok();
                            }
                        }
                    }
                    let json = serde_json::to_string(&p).unwrap_or_else(|_| "{}".into());
                    let _ = app.db.with(|c| {
                        c.execute(
                            "UPDATE files SET probe = ?, duration = COALESCE(?, duration), still = COALESCE(still, ?) WHERE id = ?",
                            params![json, p.duration, frame, id],
                        )
                    });
                }
                continue;
            }
        }
        if let Some(ffmpeg) = tools.ffmpeg {
            // Когда новых файлов нет: лёгкие постеры для рядов, потом кадры для перемотки.
            if thumbs(&app, &ffmpeg).await || trickplay_one(&app, &ffmpeg).await {
                continue;
            }
        }
        let _ = tokio::time::timeout(Duration::from_secs(900), app.probe_kick.notified()).await;
    }
}

/// Уменьшенные постеры (342 px) — ряды на главной грузятся в разы быстрее. true — что-то сделано.
async fn thumbs(app: &Arc<App>, ffmpeg: &std::path::Path) -> bool {
    let todo: Vec<(i64, String)> = app
        .db
        .with(|c| {
            let mut st =
                c.prepare("SELECT id, poster FROM titles WHERE poster IS NOT NULL AND poster_sm IS NULL LIMIT 20")?;
            let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            r.collect()
        })
        .unwrap_or_default();
    for (id, poster) in &todo {
        let small = match app.images.path(poster) {
            Some(src) => probe::scale_image(ffmpeg, &src, 342).await.ok().and_then(|b| app.images.save(&b).ok()),
            None => None,
        };
        // Не вышло — берём исходный, чтобы не пробовать по кругу.
        let small = small.unwrap_or_else(|| poster.clone());
        let _ = app.db.with(|c| c.execute("UPDATE titles SET poster_sm = ? WHERE id = ?", params![small, id]));
    }
    !todo.is_empty()
}

/// Кадры для полосы перемотки, как у Netflix: раз в 10 секунд, листы 10×10 по 240 px.
/// Декодируются только ключевые кадры — быстро даже для длинного фильма. true — что-то сделано.
async fn trickplay_one(app: &Arc<App>, ffmpeg: &std::path::Path) -> bool {
    if app.db.setting_or("media.trickplay", "1") != "1" {
        return false;
    }
    let next: Option<(i64, String, String)> = app
        .db
        .with(|c| {
            c.query_row(
                "SELECT id, path, probe FROM files WHERE trickplay IS NULL AND missing = 0 AND probe IS NOT NULL ORDER BY added_at DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
        })
        .ok()
        .flatten();
    let Some((id, path, probe)) = next else { return false };
    let p: probe::Probe = serde_json::from_str(&probe).unwrap_or_default();
    let dir = app.data.join("trickplay").join(id.to_string());
    let result = match (&p.video, p.duration) {
        (Some(v), Some(d)) if v.width > 0 && v.height > 0 && d >= 10.0 => {
            let _ = std::fs::remove_dir_all(&dir);
            probe::trickplay(ffmpeg, std::path::Path::new(&path), &dir, v.width, v.height, d).await.ok()
        }
        _ => None,
    };
    let json = result.map(|t| serde_json::to_string(&t).unwrap_or_default()).unwrap_or_else(|| "{}".into());
    let _ = app.db.with(|c| c.execute("UPDATE files SET trickplay = ? WHERE id = ?", params![json, id]));
    true
}
