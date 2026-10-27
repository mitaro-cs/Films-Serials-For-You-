//! ffprobe/ffmpeg: дорожки файла, длительность, кадр для превью.
//! Без ffmpeg всё работает, просто браузер играет только то, что умеет сам.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Probe {
    pub format: String,
    pub duration: Option<f64>,
    pub video: Option<Video>,
    pub audio: Vec<Track>,
    pub subs: Vec<Track>,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Chapter {
    pub start: f64,
    pub end: f64,
    pub title: String,
}

/// Заставка и титры по названиям глав (MKV с главами «Opening», «Intro», «Заставка», «Credits»…).
pub fn chapter_markers(chapters: &[Chapter], duration: Option<f64>) -> (Option<(f64, f64)>, Option<f64>) {
    let is = |t: &str, words: &[&str]| {
        let t = t.to_lowercase();
        let toks: Vec<&str> = t.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
        words.iter().any(|w| if w.contains(' ') { t.contains(w) } else { toks.contains(w) })
    };
    let intro = chapters
        .iter()
        .find(|c| is(&c.title, &["intro", "opening", "op", "заставка", "вступление", "начальные титры"]))
        .filter(|c| c.end > c.start && c.end - c.start < 300.0)
        .map(|c| (c.start, c.end));
    let credits = chapters
        .iter()
        .rev()
        .find(|c| {
            is(&c.title, &["credits", "ending", "ed", "end credits", "титры", "финальные титры", "конечные титры"])
        })
        .map(|c| c.start)
        .filter(|s| duration.is_none_or(|d| *s > d * 0.5));
    (intro, credits)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Video {
    pub codec: String,
    pub width: i64,
    pub height: i64,
    #[serde(default)]
    pub pix_fmt: String,
    /// Передаточная функция цвета: `smpte2084` (HDR10), `arib-std-b67` (HLG) - это HDR.
    #[serde(default)]
    pub transfer: String,
}

impl Video {
    /// HDR: при перекодировании в обычный H.264 нужен перевод цвета, иначе картинка блёклая.
    pub fn hdr(&self) -> bool {
        matches!(self.transfer.as_str(), "smpte2084" | "arib-std-b67")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Track {
    /// Номер среди дорожек того же типа (для `-map 0:a:N`).
    pub index: usize,
    pub codec: String,
    pub lang: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub channels: i64,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub forced: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Tools {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Где искать ffmpeg: своя настройка, рядом с программой, PATH и обычные места установки
/// (у приложений на Mac PATH из терминала нет - Homebrew нужно искать самим).
pub fn locate(name: &str, configured: Option<&str>) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(c) = configured.filter(|c| !c.trim().is_empty()) {
        let p = PathBuf::from(c.trim());
        if p.is_file() {
            // Указали ffmpeg - ffprobe лежит рядом.
            let sibling = p.with_file_name(exe(name));
            return sibling.is_file().then_some(sibling);
        }
        dirs.push(p);
    }
    if let Ok(me) = std::env::current_exe() {
        if let Some(d) = me.parent() {
            dirs.push(d.to_path_buf());
            dirs.push(d.join("resources"));
            dirs.push(d.join("../Resources/resources"));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/opt/local/bin"].map(PathBuf::from));
    if let Some(la) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(&la).join("Microsoft/WinGet/Links"));
    }
    for root in ["C:/ffmpeg/bin", "C:/Program Files/ffmpeg/bin"] {
        dirs.push(PathBuf::from(root));
    }
    dirs.into_iter().map(|d| d.join(exe(name))).find(|p| p.is_file())
}

impl Tools {
    pub fn find(configured: Option<&str>) -> Self {
        Self { ffmpeg: locate("ffmpeg", configured), ffprobe: locate("ffprobe", configured) }
    }
}

/// ffmpeg и ffprobe - всегда ниже обычного приоритета: компьютер хоста - это ещё и чей-то
/// рабочий компьютер, интерфейс и другие программы не должны тормозить из-за просмотра.
pub fn command(bin: &Path) -> Command {
    prioritized(bin, 10, 0x0000_4000) // BELOW_NORMAL_PRIORITY_CLASS
}

/// Фоновая работа (кадры, картинки): ещё ниже - только когда компьютер свободен.
pub fn background(bin: &Path) -> Command {
    prioritized(bin, 19, 0x0000_0040) // IDLE_PRIORITY_CLASS
}

#[allow(unused_variables)]
fn prioritized(bin: &Path, nice: u8, class: u32) -> Command {
    #[cfg(unix)]
    let mut c = {
        // `nice` заменяет себя программой (exec): тот же процесс, его и остановим.
        let tool = Path::new("/usr/bin/nice");
        if tool.is_file() {
            let mut c = Command::new(tool);
            c.args(["-n", &nice.to_string()]).arg(bin);
            c
        } else {
            Command::new(bin)
        }
    };
    #[cfg(not(unix))]
    let mut c = Command::new(bin);
    c.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        // Без чёрного окна консоли на каждый запуск (CREATE_NO_WINDOW) и с низким приоритетом.
        c.creation_flags(0x0800_0000 | class);
    }
    c
}

pub async fn probe(ffprobe: &Path, file: &Path) -> anyhow::Result<Probe> {
    let out = tokio::time::timeout(
        Duration::from_secs(60),
        command(ffprobe)
            .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", "-show_chapters"])
            .arg(file)
            .stderr(Stdio::null())
            .output(),
    )
    .await??;
    anyhow::ensure!(out.status.success(), "ffprobe не смог прочитать файл");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    Ok(from_json(&v))
}

pub fn from_json(v: &serde_json::Value) -> Probe {
    let s = |x: &serde_json::Value| x.as_str().map(String::from);
    let mut p = Probe {
        format: v["format"]["format_name"].as_str().unwrap_or_default().to_string(),
        duration: v["format"]["duration"].as_str().and_then(|d| d.parse().ok()),
        ..Default::default()
    };
    for ch in v["chapters"].as_array().into_iter().flatten() {
        let t = |k: &str| ch[k].as_str().and_then(|s| s.parse::<f64>().ok());
        if let (Some(start), Some(end)) = (t("start_time"), t("end_time")) {
            let title = ch["tags"]["title"].as_str().unwrap_or_default().to_string();
            p.chapters.push(Chapter { start, end, title });
        }
    }
    let (mut na, mut ns) = (0, 0);
    for st in v["streams"].as_array().into_iter().flatten() {
        let codec = st["codec_name"].as_str().unwrap_or_default().to_string();
        let tags = &st["tags"];
        let disp = &st["disposition"];
        match st["codec_type"].as_str() {
            Some("video") if p.video.is_none() && disp["attached_pic"].as_i64() != Some(1) => {
                p.video = Some(Video {
                    codec,
                    width: st["width"].as_i64().unwrap_or(0),
                    height: st["height"].as_i64().unwrap_or(0),
                    pix_fmt: st["pix_fmt"].as_str().unwrap_or_default().to_string(),
                    transfer: st["color_transfer"].as_str().unwrap_or_default().to_string(),
                });
            }
            Some(kind @ ("audio" | "subtitle")) => {
                let n = if kind == "audio" { &mut na } else { &mut ns };
                let t = Track {
                    index: *n,
                    codec,
                    lang: s(&tags["language"]).filter(|l| l != "und"),
                    title: s(&tags["title"]),
                    channels: st["channels"].as_i64().unwrap_or(0),
                    default: disp["default"].as_i64() == Some(1),
                    forced: disp["forced"].as_i64() == Some(1),
                };
                *n += 1;
                if kind == "audio" {
                    p.audio.push(t)
                } else {
                    p.subs.push(t)
                }
            }
            _ => {}
        }
    }
    p
}

/// Кадр из середины начала фильма - превью серии или фон, если другого нет.
pub async fn frame(ffmpeg: &Path, file: &Path, at: f64, width: u32) -> anyhow::Result<Vec<u8>> {
    let out = tokio::time::timeout(
        Duration::from_secs(60),
        command(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{at:.2}"), "-i"])
            .arg(file)
            .args([
                "-frames:v",
                "1",
                "-vf",
                &format!("scale={width}:-2"),
                "-q:v",
                "4",
                "-f",
                "image2",
                "-c:v",
                "mjpeg",
                "pipe:1",
            ])
            .stderr(Stdio::null())
            .output(),
    )
    .await??;
    anyhow::ensure!(out.status.success() && !out.stdout.is_empty(), "ffmpeg не сделал кадр");
    Ok(out.stdout)
}

/// Картинка, сжатая до 24×36 точек, пикселями RGB - для цвета карточки.
pub async fn tiny_rgb(ffmpeg: &Path, src: &Path) -> anyhow::Result<Vec<u8>> {
    let out = tokio::time::timeout(
        Duration::from_secs(30),
        command(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(src)
            .args(["-vf", "scale=24:36", "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
            .stderr(Stdio::null())
            .output(),
    )
    .await??;
    anyhow::ensure!(out.status.success() && out.stdout.len() == 24 * 36 * 3, "ffmpeg не дал пиксели");
    Ok(out.stdout)
}

/// Квадратная картинка `size`×`size` (середина кадра) - для фото профиля.
pub async fn square_image(ffmpeg: &Path, src: &Path, size: u32) -> anyhow::Result<Vec<u8>> {
    let out = tokio::time::timeout(
        Duration::from_secs(30),
        command(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(src)
            .args([
                // Без метаданных: в фото с телефона бывают место съёмки и модель (EXIF).
                "-map_metadata",
                "-1",
                "-vf",
                &format!("crop='min(iw,ih)':'min(iw,ih)',scale={size}:{size}"),
                "-q:v",
                "4",
                "-frames:v",
                "1",
                "-f",
                "image2",
                "-c:v",
                "mjpeg",
                "pipe:1",
            ])
            .stderr(Stdio::null())
            .output(),
    )
    .await??;
    anyhow::ensure!(out.status.success() && !out.stdout.is_empty(), "ffmpeg не обрезал картинку");
    Ok(out.stdout)
}

/// Картинка меньшего размера (JPEG) - для лёгких постеров.
pub async fn scale_image(ffmpeg: &Path, src: &Path, width: u32) -> anyhow::Result<Vec<u8>> {
    let out = tokio::time::timeout(
        Duration::from_secs(30),
        command(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(src)
            .args([
                // Без метаданных: в фото с телефона бывают место съёмки и модель (EXIF).
                "-map_metadata",
                "-1",
                "-vf",
                &format!("scale='min({width},iw)':-2"),
                "-q:v",
                "4",
                "-frames:v",
                "1",
                "-f",
                "image2",
                "-c:v",
                "mjpeg",
                "pipe:1",
            ])
            .stderr(Stdio::null())
            .output(),
    )
    .await??;
    anyhow::ensure!(out.status.success() && !out.stdout.is_empty(), "ffmpeg не уменьшил картинку");
    Ok(out.stdout)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trickplay {
    /// Секунд между кадрами.
    pub interval: u32,
    pub width: u32,
    pub height: u32,
    pub cols: u32,
    pub rows: u32,
    /// Всего кадров и листов.
    pub count: u32,
    pub sheets: u32,
}

pub async fn trickplay(
    ffmpeg: &Path,
    file: &Path,
    dir: &Path,
    w: i64,
    h: i64,
    duration: f64,
) -> anyhow::Result<Trickplay> {
    let (interval, width, cols, rows) = (10u32, 240u32, 10u32, 10u32);
    let height = ((width as f64 * h as f64 / w as f64 / 2.0).round() as u32 * 2).max(2);
    std::fs::create_dir_all(dir)?;
    let status = tokio::time::timeout(
        Duration::from_secs(900),
        background(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-threads", "1", "-skip_frame", "nokey", "-i"])
            .arg(file)
            .args(["-an", "-sn", "-dn", "-vf"])
            .arg(format!("fps=1/{interval},scale={width}:{height},tile={cols}x{rows}"))
            .args(["-fps_mode", "vfr", "-q:v", "5", "-threads", "1", "-filter_threads", "1", "-start_number", "0"])
            .arg(dir.join("%d.jpg"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
    )
    .await??;
    anyhow::ensure!(status.success(), "ffmpeg не сделал кадры");
    let sheets = std::fs::read_dir(dir)?
        .filter(|e| e.as_ref().is_ok_and(|e| e.path().extension().is_some_and(|x| x == "jpg")))
        .count() as u32;
    anyhow::ensure!(sheets > 0, "нет кадров");
    let count = ((duration / interval as f64).ceil() as u32).min(sheets * cols * rows);
    Ok(Trickplay { interval, width, height, cols, rows, count, sheets })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tracks() {
        let v = serde_json::json!({
            "format": {"format_name": "matroska,webm", "duration": "5400.5"},
            "streams": [
                {"codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080, "pix_fmt": "yuv420p"},
                {"codec_type": "audio", "codec_name": "ac3", "channels": 6, "tags": {"language": "rus", "title": "Дубляж"}, "disposition": {"default": 1}},
                {"codec_type": "audio", "codec_name": "aac", "channels": 2, "tags": {"language": "eng"}},
                {"codec_type": "subtitle", "codec_name": "subrip", "tags": {"language": "rus"}, "disposition": {"forced": 1}}
            ]
        });
        let p = from_json(&v);
        assert_eq!(p.duration, Some(5400.5));
        assert_eq!(p.video.unwrap().codec, "h264");
        assert_eq!(p.audio.len(), 2);
        assert_eq!(p.audio[1].index, 1);
        assert_eq!(p.audio[0].title.as_deref(), Some("Дубляж"));
        assert!(p.subs[0].forced);
    }

    #[test]
    fn markers_from_chapters() {
        let ch = |s: f64, e: f64, t: &str| Chapter { start: s, end: e, title: t.into() };
        let list = vec![
            ch(0.0, 60.0, "Пролог"),
            ch(60.0, 150.0, "Opening"),
            ch(150.0, 1300.0, "Part A"),
            ch(1300.0, 1400.0, "Ending"),
        ];
        assert_eq!(chapter_markers(&list, Some(1400.0)), (Some((60.0, 150.0)), Some(1300.0)));
        assert_eq!(chapter_markers(&[ch(0.0, 10.0, "Chapter 1")], Some(100.0)), (None, None));
        // «Титры» в начале фильма - это не финальные титры.
        assert_eq!(chapter_markers(&[ch(5.0, 60.0, "Титры")], Some(5000.0)).1, None);
    }
}
