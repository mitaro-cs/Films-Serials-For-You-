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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Video {
    pub codec: String,
    pub width: i64,
    pub height: i64,
    #[serde(default)]
    pub pix_fmt: String,
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
/// (у приложений на Mac PATH из терминала нет — Homebrew нужно искать самим).
pub fn locate(name: &str, configured: Option<&str>) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(c) = configured.filter(|c| !c.trim().is_empty()) {
        let p = PathBuf::from(c.trim());
        if p.is_file() {
            // Указали ffmpeg — ffprobe лежит рядом.
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

pub fn command(bin: &Path) -> Command {
    let mut c = Command::new(bin);
    c.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        // Без чёрного окна консоли на каждый запуск.
        c.creation_flags(0x0800_0000);
    }
    c
}

pub async fn probe(ffprobe: &Path, file: &Path) -> anyhow::Result<Probe> {
    let out = tokio::time::timeout(
        Duration::from_secs(60),
        command(ffprobe)
            .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
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

/// Кадр из середины начала фильма — превью серии или фон, если другого нет.
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
}
