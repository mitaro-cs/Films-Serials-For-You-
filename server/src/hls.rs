//! Адаптивный поток HLS, как у Netflix: качество подстраивается под скорость интернета зрителя,
//! перемотка - сразу к нужному кусочку. Кусочки по 6 секунд перекодируются ffmpeg по запросу:
//! плейлист известен заранее (длительность из ffprobe), ключевые кадры ставятся ровно на
//! границах кусочков, поэтому перезапуск ffmpeg с любого места даёт те же кусочки.
//!
//! Задача ffmpeg живёт, пока зритель смотрит: убегает больше чем на 4 минуты вперёд или
//! зрителя нет минуту - останавливается; старые кусочки удаляются.

use crate::error::{ApiError, ApiResult};
use crate::App;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::OwnedSemaphorePermit;

pub const SEG: f64 = 6.0;
const AHEAD_MAX: u64 = 40;
const KEEP_BEHIND: u64 = 10;
const IDLE: Duration = Duration::from_secs(60);

/// Ступени качества: высота кадра и битрейт видео (кбит/с). 4K и 1440p - только ручным выбором
/// (`AUTO_MAX`): перекодировать их на лету тяжело, автокачество их не берёт.
const LADDER: &[(i64, u32)] = &[(2160, 16000), (1440, 9000), (1080, 5000), (720, 2800), (480, 1200), (360, 700)];
/// Выше этого автокачество (`master`) не поднимается.
const AUTO_MAX: i64 = 1080;

/// Ступени для источника: не выше исходного; совсем маленький файл - одна ступень своей высоты.
pub fn levels(source_height: i64) -> Vec<i64> {
    let mut l: Vec<i64> = LADDER.iter().map(|(h, _)| *h).filter(|h| *h <= source_height.max(360)).collect();
    if source_height < 360 {
        l = vec![source_height - source_height % 2];
    }
    l
}

fn bitrate(h: i64) -> u32 {
    LADDER.iter().find(|(lh, _)| *lh <= h).map(|(_, b)| *b).unwrap_or(500)
}

pub fn segments(duration: f64) -> u64 {
    (duration / SEG).ceil().max(1.0) as u64
}

pub fn master(source_height: i64, audio: usize) -> String {
    let mut s = String::from("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-INDEPENDENT-SEGMENTS\n");
    for h in levels(source_height).into_iter().filter(|h| *h <= AUTO_MAX) {
        let w = (h * 16 / 9) & !1;
        let bw = (bitrate(h) + 160) * 1100;
        s.push_str(&format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={bw},RESOLUTION={w}x{h},CODECS=\"avc1.640028,mp4a.40.2\"\n{h}/index.m3u8?audio={audio}\n"
        ));
    }
    s
}

pub fn variant(duration: f64, audio: usize) -> String {
    let n = segments(duration);
    let mut s = format!(
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-TARGETDURATION:{}\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-INDEPENDENT-SEGMENTS\n",
        SEG as u64
    );
    for i in 0..n {
        let len = if i + 1 == n { duration - SEG * i as f64 } else { SEG };
        s.push_str(&format!("#EXTINF:{len:.6},\n{i}.ts?audio={audio}\n"));
    }
    s.push_str("#EXT-X-ENDLIST\n");
    s
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Job {
    pub file: i64,
    pub height: i64,
    pub audio: usize,
}

struct Running {
    child: tokio::process::Child,
    start: u64,
    requested: u64,
    seen: Instant,
    _permit: OwnedSemaphorePermit,
}

#[derive(Default)]
pub struct Hls {
    jobs: Mutex<HashMap<Job, Running>>,
}

fn dir_of(app: &App, j: &Job) -> PathBuf {
    app.data.join("cache/hls").join(format!("{}_{}_{}", j.file, j.height, j.audio))
}

/// Самый поздний готовый кусочек (ffmpeg пишет во временный файл и переименовывает).
fn produced(dir: &Path) -> Option<u64> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".ts")?.parse::<u64>().ok())
        .max()
}

/// Умеет ли этот ffmpeg переводить HDR в обычный цвет (фильтры zscale и tonemap) - спрашиваем один раз.
pub fn tonemap_ok(ffmpeg: &Path) -> bool {
    static KNOWN: std::sync::OnceLock<Mutex<HashMap<PathBuf, bool>>> = std::sync::OnceLock::new();
    let known = KNOWN.get_or_init(Default::default);
    if let Some(ok) = known.lock().get(ffmpeg) {
        return *ok;
    }
    let ok = std::process::Command::new(ffmpeg)
        .args(["-hide_banner", "-filters"])
        .output()
        .map(|o| {
            let list = String::from_utf8_lossy(&o.stdout);
            list.contains(" zscale ") && list.contains(" tonemap ")
        })
        .unwrap_or(false);
    known.lock().insert(ffmpeg.to_path_buf(), ok);
    ok
}

/// Перевод HDR (PQ/HLG, BT.2020) в обычный BT.709: без него перекодированная 4K HDR картинка блёклая.
const TONEMAP: &str =
    "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv";

#[allow(clippy::too_many_arguments)]
pub fn args(
    input: &Path,
    dir: &Path,
    height: i64,
    source_height: i64,
    audio: usize,
    start: u64,
    tonemap: bool,
) -> Vec<String> {
    let br = bitrate(height);
    let t0 = start as f64 * SEG;
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin"].map(String::from).to_vec();
    if start > 0 {
        a.extend(["-ss".into(), format!("{t0:.3}")]);
    }
    a.extend(["-i".into(), input.to_string_lossy().into_owned()]);
    a.extend(["-map".into(), "0:v:0".into(), "-map".into(), format!("0:a:{audio}?")]);
    a.extend(["-sn", "-dn", "-map_metadata", "-1", "-map_chapters", "-1"].map(String::from));
    let scale =
        if height < source_height { format!("scale=-2:{height}") } else { "scale=trunc(iw/2)*2:trunc(ih/2)*2".into() };
    let vf = if tonemap { format!("{scale},{TONEMAP},format=yuv420p") } else { scale };
    a.extend(["-vf".into(), vf]);
    a.extend(
        [
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-profile:v",
            "high",
            "-level",
            if height > AUTO_MAX { "5.1" } else { "4.1" },
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "23",
        ]
        .map(String::from),
    );
    // Не больше 4 потоков: этого хватает на 1080p быстрее просмотра, а компьютер хоста не раскручивает
    // вентилятор на все ядра. 1440p и 4K (только по выбору зрителя) - на все ядра, иначе не успеть.
    a.extend(["-threads".into(), if height > AUTO_MAX { "0".into() } else { "4".into() }]);
    a.extend(["-maxrate".into(), format!("{br}k"), "-bufsize".into(), format!("{}k", br * 2)]);
    a.extend(["-force_key_frames".into(), format!("expr:gte(t,n_forced*{SEG})"), "-sc_threshold".into(), "0".into()]);
    a.extend(["-c:a", "aac", "-ac", "2", "-b:a", "160k"].map(String::from));
    a.extend(["-output_ts_offset".into(), format!("{t0:.3}")]);
    a.extend(["-f", "hls", "-hls_time"].map(String::from));
    a.push(format!("{SEG}"));
    a.extend(
        ["-hls_list_size", "0", "-hls_segment_type", "mpegts", "-hls_flags", "temp_file+independent_segments"]
            .map(String::from),
    );
    a.extend(["-start_number".into(), start.to_string()]);
    a.extend(["-hls_segment_filename".into(), dir.join("%d.ts").to_string_lossy().into_owned()]);
    a.push(dir.join("index.m3u8").to_string_lossy().into_owned());
    a
}

impl Job {
    /// Готовый кусочек `n`: ждёт текущую задачу или запускает новую с этого места.
    pub async fn segment(
        &self,
        app: &Arc<App>,
        ffmpeg: &Path,
        input: &Path,
        source_height: i64,
        hdr: bool,
        n: u64,
    ) -> ApiResult<PathBuf> {
        let dir = dir_of(app, self);
        let file = dir.join(format!("{n}.ts"));
        let deadline = Instant::now() + Duration::from_secs(40);
        loop {
            if file.is_file() {
                if let Some(r) = app.hls.jobs.lock().get_mut(self) {
                    r.requested = r.requested.max(n);
                    r.seen = Instant::now();
                }
                return Ok(file);
            }
            if Instant::now() > deadline {
                return Err(ApiError::internal("кусочек HLS не появился за 40 секунд"));
            }
            {
                let mut jobs = app.hls.jobs.lock();
                let usable = jobs.get_mut(self).is_some_and(|r| {
                    let alive = r.child.try_wait().ok().flatten().is_none();
                    let near = r.start <= n && n <= produced(&dir).unwrap_or(r.start).max(r.start) + 4;
                    if alive && near {
                        r.seen = Instant::now();
                    }
                    alive && near
                });
                if !usable {
                    jobs.remove(self); // старая задача (не там или завершилась) останавливается
                                       // Check and register under the same lock: concurrent viewers must
                                       // share one encoder, not race to replace each other's process.
                    let permit = app.transcode.clone().try_acquire_owned().map_err(|_| crate::api::busy())?;
                    std::fs::create_dir_all(&dir)?;
                    let child = crate::library::probe::command(ffmpeg)
                        .args(args(input, &dir, self.height, source_height, self.audio, n, hdr && tonemap_ok(ffmpeg)))
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()?;
                    jobs.insert(
                        *self,
                        Running { child, start: n, requested: n, seen: Instant::now(), _permit: permit },
                    );
                }
            }
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
    }
}

/// Раз в 2 секунды: убегающие вперёд и брошенные задачи останавливаются, старые кусочки удаляются.
pub async fn janitor(app: Arc<App>) {
    let root = app.data.join("cache/hls");
    let _ = std::fs::remove_dir_all(&root);
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let mut gone: Vec<(Job, bool)> = Vec::new();
        {
            let mut jobs = app.hls.jobs.lock();
            jobs.retain(|j, r| {
                let dir = dir_of(&app, j);
                let ahead = produced(&dir).unwrap_or(0).saturating_sub(r.requested);
                let idle = r.seen.elapsed() > IDLE;
                if idle || ahead > AHEAD_MAX || r.child.try_wait().ok().flatten().is_some() {
                    gone.push((*j, idle));
                    false
                } else {
                    true
                }
            });
        }
        for (j, idle) in gone {
            if idle {
                let _ = std::fs::remove_dir_all(dir_of(&app, &j));
            }
        }
        // Кусочки далеко позади зрителя больше не нужны.
        let active: Vec<(Job, u64)> = app.hls.jobs.lock().iter().map(|(j, r)| (*j, r.requested)).collect();
        for (j, requested) in active {
            let dir = dir_of(&app, &j);
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let n =
                        e.file_name().to_str().and_then(|s| s.strip_suffix(".ts")).and_then(|s| s.parse::<u64>().ok());
                    if n.is_some_and(|n| n + KEEP_BEHIND < requested) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlists() {
        assert_eq!(levels(2160), vec![2160, 1440, 1080, 720, 480, 360]);
        assert_eq!(levels(1600), vec![1440, 1080, 720, 480, 360], "4K «скоуп» 3840×1600");
        assert!(!master(2160, 0).contains("2160/") && master(2160, 0).contains("1080/"), "авто - не выше 1080p");
        let hdr = args(Path::new("a.mkv"), Path::new("d"), 1080, 2160, 0, 0, true);
        let vf = &hdr[hdr.iter().position(|x| x == "-vf").unwrap() + 1];
        assert!(
            vf.starts_with("scale=-2:1080,zscale=t=linear") && vf.ends_with("format=yuv420p"),
            "HDR - в обычный цвет: {vf}"
        );
        assert!(args(Path::new("a.mkv"), Path::new("d"), 2160, 2160, 0, 0, false)
            .windows(2)
            .any(|w| w == ["-level", "5.1"]));
        assert_eq!(levels(720), vec![720, 480, 360]);
        assert_eq!(levels(240), vec![240]);
        let v = variant(13.5, 1);
        assert_eq!(v.matches("#EXTINF").count(), 3);
        assert!(v.contains("#EXTINF:1.500000,\n2.ts?audio=1"));
        assert!(v.ends_with("#EXT-X-ENDLIST\n"));
        let m = master(1080, 0);
        assert!(m.contains("RESOLUTION=1920x1080") && m.contains("720/index.m3u8?audio=0"));
    }
}
