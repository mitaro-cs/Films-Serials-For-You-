//! Фоновые задачи библиотеки: обход папок и разбор файлов через ffprobe.

use super::probe;
use super::scan::{scan, ScanStats};
use crate::App;
use rusqlite::params;
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
        let _ = tokio::time::timeout(Duration::from_secs(900), app.probe_kick.notified()).await;
    }
}
