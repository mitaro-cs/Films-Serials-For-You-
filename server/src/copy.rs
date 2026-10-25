//! Готовая копия для браузера без перекодирования: видео H.264/HEVC из MKV, AVI и т. п.
//! копируется как есть, звук при надобности - в AAC, на выходе HLS из кусочков fMP4.
//!
//! Раньше «Оригинал» отдавался одним потоком ffmpeg без Range. Safari (окно приложения на Mac,
//! iPhone) запрашивает видео кусками по Range - на каждый запрос запускался новый ffmpeg с
//! начала, и через минуту-другую плеер падал: «Видео не воспроизводится». Копия делается одним
//! процессом примерно в 50-100 раз быстрее просмотра (процессор почти не нагружается), кусочки
//! - обычные файлы, перемотка и докачка - как у любого HLS.
//!
//! Копии живут в `cache/copy`, пока к ним обращаются: брошенная за 10 минут задача
//! останавливается, копия без зрителей 3 часа удаляется; не больше `KEEP` копий сразу.

use crate::error::{ApiError, ApiResult};
use crate::App;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

const IDLE_JOB: Duration = Duration::from_secs(600);
const IDLE_COPY: Duration = Duration::from_secs(3 * 3600);
const KEEP: usize = 4;
/// Сколько готового нужно впереди места, с которого начинают смотреть.
const LEAD: f64 = 18.0;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Key {
    pub file: i64,
    pub audio: usize,
}

struct Entry {
    child: Option<tokio::process::Child>,
    seen: Instant,
}

#[derive(Default)]
pub struct Copies {
    jobs: Mutex<HashMap<Key, Entry>>,
    start: Mutex<()>,
}

pub struct Source<'a> {
    pub input: &'a Path,
    pub hevc: bool,
    pub copy_audio: bool,
}

fn dir_of(app: &App, k: &Key) -> PathBuf {
    app.data.join("cache/copy").join(format!("{}_{}", k.file, k.audio))
}

pub fn args(input: &Path, audio: usize, hevc: bool, copy_audio: bool) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin", "-i"].map(String::from).to_vec();
    a.push(input.to_string_lossy().into_owned());
    a.extend(["-map".into(), "0:v:0".into(), "-map".into(), format!("0:a:{audio}?")]);
    a.extend(["-sn", "-dn", "-map_metadata", "-1", "-map_chapters", "-1", "-c:v", "copy"].map(String::from));
    if hevc {
        // Safari играет HEVC только с меткой hvc1.
        a.extend(["-tag:v", "hvc1"].map(String::from));
    }
    if copy_audio {
        a.extend(["-c:a", "copy"].map(String::from));
    } else {
        a.extend(["-c:a", "aac", "-b:a", "192k", "-ac", "2"].map(String::from));
    }
    a.extend(
        [
            "-max_muxing_queue_size",
            "1024",
            "-f",
            "hls",
            "-hls_time",
            "6",
            "-hls_list_size",
            "0",
            "-hls_playlist_type",
            "event",
            "-hls_segment_type",
            "fmp4",
            "-hls_fmp4_init_filename",
            "init.mp4",
            "-hls_flags",
            "temp_file+independent_segments",
            "-hls_segment_filename",
        ]
        .map(String::from),
    );
    // Имена - относительно папки копии (ffmpeg запускается в ней): в плейлисте только имена
    // кусочков, без путей с диска (на Windows ffmpeg иначе мог записать `C:\…`).
    a.extend(["s%d.m4s", "index.m3u8"].map(String::from));
    a
}

/// Сколько секунд уже готово и закончена ли копия.
pub fn progress(playlist: &str) -> (f64, bool) {
    let total = playlist
        .lines()
        .filter_map(|l| l.strip_prefix("#EXTINF:"))
        .filter_map(|l| l.split(',').next()?.parse::<f64>().ok())
        .sum();
    (total, playlist.contains("#EXT-X-ENDLIST"))
}

/// Плейлист для плеера: начинать с начала (а не с «живого края», как у незаконченного).
pub fn for_player(playlist: &str) -> String {
    playlist.replacen("#EXTM3U\n", "#EXTM3U\n#EXT-X-START:TIME-OFFSET=0,PRECISE=YES\n", 1)
}

impl Copies {
    fn touch(&self, k: &Key) {
        if let Some(e) = self.jobs.lock().get_mut(k) {
            e.seen = Instant::now();
        }
    }

    /// Плейлист, в котором готово место `from` (+18 с); запускает копию, если её нет.
    pub async fn playlist(
        &self,
        app: &Arc<App>,
        ffmpeg: &Path,
        k: Key,
        src: Source<'_>,
        from: f64,
    ) -> ApiResult<String> {
        let dir = dir_of(app, &k);
        let index = dir.join("index.m3u8");
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            let text = std::fs::read_to_string(&index).unwrap_or_default();
            let (ready, done) = progress(&text);
            let running = {
                let mut jobs = self.jobs.lock();
                let e = jobs.entry(k).or_insert(Entry { child: None, seen: Instant::now() });
                e.seen = Instant::now();
                e.child.as_mut().is_some_and(|c| c.try_wait().ok().flatten().is_none())
            };
            if done || (running && ready >= from + LEAD) {
                return Ok(for_player(&text));
            }
            if !running {
                let _start = self.start.lock();
                // Another request may have started this copy since the check above.
                if self
                    .jobs
                    .lock()
                    .get_mut(&k)
                    .and_then(|e| e.child.as_mut())
                    .is_some_and(|c| c.try_wait().ok().flatten().is_none())
                {
                    continue;
                }
                // A short copy may already have completed while waiting for the lock.
                let latest = std::fs::read_to_string(&index).unwrap_or_default();
                if progress(&latest).1 {
                    return Ok(for_player(&latest));
                }
                if !text.is_empty() && !done {
                    // Прежняя задача оборвалась на середине - та копия негодна.
                    let _ = std::fs::remove_dir_all(&dir);
                    if self.jobs.lock().get(&k).is_some_and(|e| e.child.is_some()) {
                        return Err(ApiError::internal("копия видео оборвалась"));
                    }
                }
                self.limit(app, &k);
                std::fs::create_dir_all(&dir)?;
                let child = crate::library::probe::command(ffmpeg)
                    .args(args(src.input, k.audio, src.hevc, src.copy_audio))
                    .current_dir(&dir)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?;
                self.jobs.lock().insert(k, Entry { child: Some(child), seen: Instant::now() });
            }
            if Instant::now() > deadline {
                // Отдаём то, что есть: плеер дочитает, когда будет готово.
                return Ok(for_player(&text));
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    /// Кусочек или начало копии (`init.mp4`, `s12.m4s`).
    pub fn part(&self, app: &App, k: Key, name: &str) -> ApiResult<PathBuf> {
        let ok = name == "init.mp4"
            || name.strip_prefix('s').and_then(|n| n.strip_suffix(".m4s")).is_some_and(|n| n.parse::<u32>().is_ok());
        if !ok {
            return Err(ApiError::not_found());
        }
        self.touch(&k);
        let p = dir_of(app, &k).join(name);
        if p.is_file() {
            Ok(p)
        } else {
            Err(ApiError::not_found())
        }
    }

    /// Не больше `KEEP` копий: перед новой удаляем самую давнюю без зрителей.
    fn limit(&self, app: &App, keep: &Key) {
        let mut jobs = self.jobs.lock();
        while jobs.len() > KEEP {
            let Some(old) = jobs.iter().filter(|(k, _)| *k != keep).min_by_key(|(_, e)| e.seen).map(|(k, _)| *k) else {
                break;
            };
            jobs.remove(&old);
            let _ = std::fs::remove_dir_all(dir_of(app, &old));
        }
    }
}

/// Раз в 30 с: брошенные задачи останавливаются, старые копии удаляются. При запуске - чистый лист.
pub async fn janitor(app: Arc<App>) {
    let _ = std::fs::remove_dir_all(app.data.join("cache/copy"));
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let mut gone = Vec::new();
        {
            let mut jobs = app.copies.jobs.lock();
            jobs.retain(|k, e| {
                let idle = e.seen.elapsed();
                if let Some(c) = e.child.as_mut() {
                    if c.try_wait().ok().flatten().is_none() && idle > IDLE_JOB {
                        gone.push(*k);
                        return false;
                    }
                }
                if idle > IDLE_COPY {
                    gone.push(*k);
                    return false;
                }
                true
            });
        }
        for k in gone {
            let _ = std::fs::remove_dir_all(dir_of(&app, &k));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_progress() {
        let p = "#EXTM3U\n#EXT-X-VERSION:7\n#EXTINF:6.000000,\ns0.m4s\n#EXTINF:4.5,\ns1.m4s\n";
        assert_eq!(progress(p), (10.5, false));
        assert!(progress(&format!("{p}#EXT-X-ENDLIST\n")).1);
        assert!(for_player(p).starts_with("#EXTM3U\n#EXT-X-START:TIME-OFFSET=0"));
        let a = args(Path::new("/m/a.mkv"), 1, true, false);
        assert!(a.windows(2).any(|w| w == ["-tag:v", "hvc1"]) && a.windows(2).any(|w| w == ["-c:a", "aac"]));
        assert!(a.contains(&"0:a:1?".to_string()));
        assert!(a.ends_with(&["s%d.m4s".to_string(), "index.m3u8".to_string()]), "без путей с диска");
    }
}
