//! Отдача видео: файл целиком с Range (перемотка, докачка), перепаковка через ffmpeg для того,
//! что браузер не играет сам (MKV, AVI, AC3, другие дорожки), и субтитры в WebVTT.

use crate::library::probe::{self, Probe};
use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::Stream;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::OwnedSemaphorePermit;
use tokio_util::io::ReaderStream;

const CHUNK: usize = 256 * 1024;

pub fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        Some("ts" | "m2ts") => "video/mp2t",
        Some("ogv") => "video/ogg",
        _ => "application/octet-stream",
    }
}

/// `bytes=a-b`, `bytes=a-`, `bytes=-n` → [start, end] включительно. Несколько диапазонов — берём первый.
pub fn parse_range(h: &str, len: u64) -> Option<Result<(u64, u64), ()>> {
    let spec = h.trim().strip_prefix("bytes=")?.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    if len == 0 {
        return Some(Err(()));
    }
    let r = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        if n == 0 {
            return Some(Err(()));
        }
        (len.saturating_sub(n), len - 1)
    } else {
        let start: u64 = a.parse().ok()?;
        let end = if b.is_empty() { len - 1 } else { b.parse::<u64>().ok()?.min(len - 1) };
        if start > end || start >= len {
            return Some(Err(()));
        }
        (start, end)
    };
    Some(Ok(r))
}

/// Файл с поддержкой Range. `download` — отдать как вложение с именем.
pub async fn file(path: &Path, headers: &HeaderMap, download: Option<&str>) -> std::io::Result<Response> {
    let mut f = tokio::fs::File::open(path).await?;
    let len = f.metadata().await?.len();
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|h| parse_range(h, len));
    let mut res = Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, mime(path))
        .header(header::CACHE_CONTROL, "private, max-age=3600");
    if let Some(name) = download {
        res = res.header(header::CONTENT_DISPOSITION, crate::util::content_disposition("attachment", name));
    }
    let (status, start, count) = match range {
        Some(Err(())) => {
            return Ok((StatusCode::RANGE_NOT_SATISFIABLE, [(header::CONTENT_RANGE, format!("bytes */{len}"))])
                .into_response());
        }
        Some(Ok((s, e))) => {
            res = res.header(header::CONTENT_RANGE, format!("bytes {s}-{e}/{len}"));
            (StatusCode::PARTIAL_CONTENT, s, e - s + 1)
        }
        None => (StatusCode::OK, 0, len),
    };
    if start > 0 {
        f.seek(std::io::SeekFrom::Start(start)).await?;
    }
    let body = Body::from_stream(ReaderStream::with_capacity(f.take(count), CHUNK));
    Ok(res
        .status(status)
        .header(header::CONTENT_LENGTH, count)
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

#[derive(Debug, Serialize)]
pub struct PlayInfo {
    /// Браузер сыграет файл как есть (с первой аудиодорожкой).
    pub direct: bool,
    /// Есть ffmpeg — можно перепаковать и выбрать дорожку.
    pub remux: bool,
    /// Понадобится перекодирование видео (тяжело для компьютера).
    pub transcode: bool,
    pub hevc: bool,
    pub duration: Option<f64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub audio: Vec<TrackInfo>,
    pub subs: Vec<SubInfo>,
}

#[derive(Debug, Serialize)]
pub struct TrackInfo {
    pub index: usize,
    pub label: String,
    pub lang: Option<String>,
    pub default: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct SubInfo {
    /// `f0` — файл рядом, `e0` — внутри видео.
    pub id: String,
    pub label: String,
    pub lang: Option<String>,
    pub forced: bool,
}

const DIRECT_VIDEO: &[&str] = &["h264", "vp8", "vp9", "av1"];
const DIRECT_AUDIO: &[&str] = &["aac", "mp3", "opus", "vorbis", "flac"];
const TEXT_SUBS: &[&str] = &["subrip", "ass", "ssa", "mov_text", "webvtt", "text"];

fn lang_name(code: &str) -> String {
    match code {
        "rus" | "ru" => "Русский",
        "eng" | "en" => "English",
        "ukr" | "uk" => "Українська",
        "jpn" | "ja" => "日本語",
        "ger" | "deu" | "de" => "Deutsch",
        "fre" | "fra" | "fr" => "Français",
        "spa" | "es" => "Español",
        "ita" | "it" => "Italiano",
        "kor" | "ko" => "한국어",
        "chi" | "zho" | "zh" => "中文",
        other => return other.to_string(),
    }
    .to_string()
}

fn channels(n: i64) -> &'static str {
    match n {
        1 => " · моно",
        2 => "",
        6 => " · 5.1",
        8 => " · 7.1",
        _ => "",
    }
}

/// Субтитры рядом с видео: `Фильм.srt`, `Фильм.ru.srt`, `Фильм.Russian.forced.vtt`, `Subs/*.srt`.
pub fn sidecar_subs(video: &Path) -> Vec<(PathBuf, Option<String>, bool)> {
    let (Some(dir), Some(stem)) = (video.parent(), video.file_stem().and_then(|s| s.to_str())) else {
        return vec![];
    };
    let stem_l = stem.to_lowercase();
    let mut out = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    for d in ["Subs", "subs", "Subtitles", "subtitles"] {
        dirs.push(dir.join(d));
    }
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut found: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        found.sort();
        for p in found {
            let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
            if !matches!(ext.as_deref(), Some("srt" | "vtt" | "ass" | "ssa")) {
                continue;
            }
            let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
            let in_subs_dir = d != dir;
            if !name.starts_with(&stem_l) && !in_subs_dir {
                continue;
            }
            let rest = name.strip_prefix(&stem_l).unwrap_or(&name);
            let parts: Vec<&str> = rest.split(['.', '_', ' ', '-']).filter(|s| !s.is_empty()).collect();
            let forced = parts.contains(&"forced");
            let lang = parts.iter().find(|s| **s != "forced" && **s != "sdh").map(|s| s.to_string());
            out.push((p, lang, forced));
        }
    }
    out
}

/// Браузер любой системы сыграет файл сам: MP4/WebM с H.264 (8 бит), VP9 или AV1 и обычным звуком.
pub fn direct_ok(video: &Path, p: &Probe) -> bool {
    let ext = video.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let Some(v) = &p.video else { return false };
    matches!(ext.as_str(), "mp4" | "m4v" | "webm" | "mov")
        && DIRECT_VIDEO.contains(&v.codec.as_str())
        && !v.pix_fmt.contains("10")
        && p.audio.first().is_none_or(|a| DIRECT_AUDIO.contains(&a.codec.as_str()))
}

pub fn play_info(video: &Path, p: Option<&Probe>, has_ffmpeg: bool) -> PlayInfo {
    let ext = video.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let container_ok = matches!(ext.as_str(), "mp4" | "m4v" | "webm" | "mov");
    let mut subs: Vec<SubInfo> = sidecar_subs(video)
        .into_iter()
        .enumerate()
        .map(|(i, (path, lang, forced))| SubInfo {
            id: format!("f{i}"),
            label: lang
                .as_deref()
                .map(lang_name)
                .unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("Субтитры").to_string())
                + if forced { " (надписи)" } else { "" },
            lang,
            forced,
        })
        .collect();
    let Some(p) = p.filter(|p| p.video.is_some() || !p.audio.is_empty()) else {
        // Не разобран (нет ffprobe): пробуем как есть.
        return PlayInfo {
            direct: container_ok,
            remux: has_ffmpeg,
            transcode: false,
            hevc: false,
            duration: None,
            width: None,
            height: None,
            audio: vec![],
            subs,
        };
    };
    let vcodec = p.video.as_ref().map(|v| v.codec.as_str()).unwrap_or("");
    let hevc = vcodec == "hevc";
    let video_ok = p.video.is_none() || DIRECT_VIDEO.contains(&vcodec);
    let audio_ok = p.audio.first().is_none_or(|a| DIRECT_AUDIO.contains(&a.codec.as_str()));
    let ten_bit = p.video.as_ref().is_some_and(|v| v.pix_fmt.contains("10"));
    if has_ffmpeg {
        for s in p.subs.iter().filter(|s| TEXT_SUBS.contains(&s.codec.as_str())) {
            subs.push(SubInfo {
                id: format!("e{}", s.index),
                label: s.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| {
                    s.lang.as_deref().map(lang_name).unwrap_or_else(|| format!("Дорожка {}", s.index + 1))
                }) + if s.forced { " (надписи)" } else { "" },
                lang: s.lang.clone(),
                forced: s.forced,
            });
        }
    }
    PlayInfo {
        direct: container_ok && (video_ok || hevc) && audio_ok && !(ten_bit && vcodec == "h264"),
        remux: has_ffmpeg,
        transcode: !(DIRECT_VIDEO.contains(&vcodec) || hevc) || (ten_bit && vcodec == "h264"),
        hevc,
        duration: p.duration,
        width: p.video.as_ref().map(|v| v.width),
        height: p.video.as_ref().map(|v| v.height),
        audio: p
            .audio
            .iter()
            .map(|a| {
                let base = a.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| {
                    a.lang.as_deref().map(lang_name).unwrap_or_else(|| format!("Дорожка {}", a.index + 1))
                });
                let base = match (&a.lang, &a.title) {
                    (Some(l), Some(t)) if !t.is_empty() => format!("{t} · {}", lang_name(l)),
                    _ => base,
                };
                TrackInfo {
                    index: a.index,
                    label: format!("{base}{}", channels(a.channels)),
                    lang: a.lang.clone(),
                    default: a.default,
                }
            })
            .collect(),
        subs,
    }
}

/// Выход ffmpeg как поток ответа. Процесс завершается вместе с потоком (зритель ушёл — ffmpeg тоже).
struct FfStream {
    inner: ReaderStream<tokio::process::ChildStdout>,
    _child: tokio::process::Child,
    _permit: OwnedSemaphorePermit,
}

impl Stream for FfStream {
    type Item = std::io::Result<axum::body::Bytes>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

pub struct RemuxOpts {
    pub start: f64,
    pub audio: usize,
    pub copy_video: bool,
    pub hevc: bool,
    pub copy_audio: bool,
    /// Высота при перекодировании (не больше исходной).
    pub max_height: i64,
}

pub fn remux_args(input: &Path, o: &RemuxOpts) -> Vec<String> {
    let mut a: Vec<String> = vec!["-hide_banner".into(), "-loglevel".into(), "error".into(), "-nostdin".into()];
    if o.start > 0.5 {
        a.extend(["-ss".into(), format!("{:.3}", o.start)]);
    }
    a.extend(["-i".into(), input.to_string_lossy().into_owned()]);
    a.extend(["-map".into(), "0:v:0?".into(), "-map".into(), format!("0:a:{}?", o.audio)]);
    a.extend(["-sn", "-dn", "-map_metadata", "-1", "-map_chapters", "-1"].map(String::from));
    if o.copy_video {
        a.extend(["-c:v".into(), "copy".into()]);
        if o.hevc {
            a.extend(["-tag:v".into(), "hvc1".into()]);
        }
    } else {
        a.extend(
            [
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "22",
                "-pix_fmt",
                "yuv420p",
                "-profile:v",
                "high",
                "-g",
                "48",
            ]
            .map(String::from),
        );
        a.extend(["-vf".into(), format!("scale=-2:'min({},ih)'", o.max_height)]);
    }
    if o.copy_audio {
        a.extend(["-c:a".into(), "copy".into()]);
    } else {
        a.extend(["-c:a", "aac", "-b:a", "192k", "-ac", "2"].map(String::from));
    }
    a.extend(["-movflags", "frag_keyframe+empty_moov+default_base_moof", "-f", "mp4", "pipe:1"].map(String::from));
    a
}

pub fn remux(ffmpeg: &Path, args: Vec<String>, permit: OwnedSemaphorePermit) -> std::io::Result<Response> {
    let mut child = probe::command(ffmpeg).args(args).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let stdout = child.stdout.take().ok_or_else(|| std::io::Error::other("нет вывода ffmpeg"))?;
    let s = FfStream { inner: ReaderStream::with_capacity(stdout, CHUNK), _child: child, _permit: permit };
    let mut res = Response::new(Body::from_stream(s));
    res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("video/mp4"));
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(res)
}

// ---------- субтитры ----------

/// Windows-1251 → UTF-8 (старые русские .srt почти всегда в ней).
pub fn decode_text(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if b.starts_with(&[0xFF, 0xFE]) || b.starts_with(&[0xFE, 0xFF]) {
        let le = b[0] == 0xFF;
        let units: Vec<u16> = b[2..]
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Ok(s) = std::str::from_utf8(b) {
        return s.to_string();
    }
    const HI: &str = "ЂЃ‚ѓ„…†‡€‰Љ‹ЊЌЋЏђ‘’“”•–—\u{98}™љ›њќћџ\u{a0}ЎўЈ¤Ґ¦§Ё©Є«¬\u{ad}®Ї°±Ііґµ¶·ё№є»јЅѕї";
    let hi: Vec<char> = HI.chars().collect();
    b.iter()
        .map(|&x| match x {
            0..=0x7F => x as char,
            0x80..=0xBF => hi[(x - 0x80) as usize],
            _ => char::from_u32(0x410 + (x as u32 - 0xC0)).unwrap_or('?'),
        })
        .collect()
}

/// SRT → WebVTT: заголовок и точки вместо запятых в метках времени.
pub fn srt_to_vtt(s: &str) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for line in s.replace("\r\n", "\n").replace('\r', "\n").lines() {
        if line.contains("-->") {
            out.push_str(&line.replace(',', "."));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Субтитры в WebVTT: файл рядом или дорожка из видео (через ffmpeg).
pub async fn subtitle(video: &Path, id: &str, ffmpeg: Option<&Path>) -> anyhow::Result<String> {
    let (kind, n) = id.split_at(1);
    let n: usize = n.parse()?;
    let convert = |path: PathBuf| async move {
        let ff = ffmpeg.ok_or_else(|| anyhow::anyhow!("для ASS-субтитров нужен ffmpeg"))?;
        let out = probe::command(ff)
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(path)
            .args(["-f", "webvtt", "pipe:1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await?;
        anyhow::ensure!(out.status.success(), "ffmpeg не смог прочитать субтитры");
        Ok::<_, anyhow::Error>(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    match kind {
        "f" => {
            let (path, _, _) =
                sidecar_subs(video).into_iter().nth(n).ok_or_else(|| anyhow::anyhow!("нет таких субтитров"))?;
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
            match ext.as_str() {
                "srt" => Ok(srt_to_vtt(&decode_text(&tokio::fs::read(&path).await?))),
                "vtt" => Ok(decode_text(&tokio::fs::read(&path).await?)),
                _ => convert(path).await,
            }
        }
        "e" => {
            let ff = ffmpeg.ok_or_else(|| anyhow::anyhow!("для встроенных субтитров нужен ffmpeg"))?;
            let out = probe::command(ff)
                .args(["-hide_banner", "-loglevel", "error", "-i"])
                .arg(video)
                .args(["-map", &format!("0:s:{n}"), "-f", "webvtt", "pipe:1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .await?;
            anyhow::ensure!(out.status.success(), "ffmpeg не смог достать субтитры");
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        }
        _ => anyhow::bail!("нет таких субтитров"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some(Ok((0, 99))));
        assert_eq!(parse_range("bytes=900-", 1000), Some(Ok((900, 999))));
        assert_eq!(parse_range("bytes=-100", 1000), Some(Ok((900, 999))));
        assert_eq!(parse_range("bytes=990-2000", 1000), Some(Ok((990, 999))));
        assert_eq!(parse_range("bytes=1000-", 1000), Some(Err(())));
        assert_eq!(parse_range("items=1-2", 1000), None);
    }

    #[test]
    fn cp1251_and_srt() {
        let b = [0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2, 0x2C, 0x20, 0xB8, 0xEB, 0xEA, 0xE0];
        assert_eq!(decode_text(&b), "Привет, ёлка");
        let vtt = srt_to_vtt("1\r\n00:00:01,000 --> 00:00:02,500\r\nПривет, мир\r\n");
        assert!(vtt.starts_with("WEBVTT"));
        assert!(vtt.contains("00:00:01.000 --> 00:00:02.500"));
        assert!(vtt.contains("Привет, мир"));
    }

    #[test]
    fn decides_direct_play() {
        let mut p = Probe { format: "mov,mp4".into(), duration: Some(10.0), ..Default::default() };
        p.video = Some(probe::Video { codec: "h264".into(), width: 1920, height: 1080, pix_fmt: "yuv420p".into() });
        p.audio.push(probe::Track { codec: "aac".into(), ..Default::default() });
        assert!(play_info(Path::new("a.mp4"), Some(&p), false).direct);
        assert!(!play_info(Path::new("a.mkv"), Some(&p), true).direct);
        p.audio[0].codec = "ac3".into();
        let i = play_info(Path::new("a.mp4"), Some(&p), true);
        assert!(!i.direct && i.remux && !i.transcode);
    }

    #[test]
    fn sidecar_languages() {
        let dir = tempfile::tempdir().unwrap();
        let v = dir.path().join("Film.mkv");
        std::fs::write(&v, b"x").unwrap();
        std::fs::write(dir.path().join("Film.ru.srt"), b"x").unwrap();
        std::fs::write(dir.path().join("Film.en.forced.srt"), b"x").unwrap();
        std::fs::write(dir.path().join("Other.srt"), b"x").unwrap();
        let s = sidecar_subs(&v);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].1.as_deref(), Some("en"));
        assert!(s[0].2);
        assert_eq!(s[1].1.as_deref(), Some("ru"));
    }
}
