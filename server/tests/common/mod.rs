#![allow(dead_code)]
//! Общее для сквозных тестов: тестовые видео через ffmpeg и HTTP-клиент с cookie.

use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

pub fn ffmpeg() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success())
}

pub fn make_video(path: &Path, audio: &str) {
    make_video_with(path, audio, 4, None);
}

/// Видео длиной `secs` секунд; `chapters` — файл FFMETADATA с главами.
pub fn make_video_with(path: &Path, audio: &str, secs: u32, chapters: Option<&Path>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut c = Command::new("ffmpeg");
    c.args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", &format!("testsrc=duration={secs}:size=320x240:rate=25")])
        .args(["-f", "lavfi", "-i", &format!("sine=duration={secs}")]);
    if let Some(meta) = chapters {
        c.arg("-i").arg(meta).args(["-map", "0:v", "-map", "1:a", "-map_chapters", "2"]);
    }
    let ok = c
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-g", "50", "-c:a", audio])
        .arg(path)
        .status()
        .unwrap()
        .success();
    assert!(ok, "ffmpeg не сделал тестовое видео");
}

pub struct Client {
    pub base: String,
    pub http: reqwest::Client,
    pub cookie: String,
}

impl Client {
    pub fn new(base: String) -> Self {
        Self {
            base,
            http: reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap(),
            cookie: String::new(),
        }
    }

    pub fn take_cookie(&mut self, res: &reqwest::Response) {
        if let Some(c) = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()) {
            self.cookie = c.split(';').next().unwrap().to_string();
        }
    }

    pub async fn get(&self, path: &str) -> reqwest::Response {
        self.http.get(format!("{}{path}", self.base)).header("cookie", &self.cookie).send().await.unwrap()
    }

    pub async fn json(&self, path: &str) -> Value {
        let r = self.get(path).await;
        assert!(r.status().is_success(), "{path}: {}", r.status());
        r.json().await.unwrap()
    }

    pub async fn send(&mut self, method: &str, path: &str, body: Value) -> reqwest::Response {
        let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
        let r = self
            .http
            .request(m, format!("{}{path}", self.base))
            .header("cookie", &self.cookie)
            .header("x-fsfy", "1")
            .json(&body)
            .send()
            .await
            .unwrap();
        self.take_cookie(&r);
        r
    }
}

/// Сервер во временном каталоге; хост вошёл из окна, описания из интернета выключены.
pub async fn start(dir: &Path) -> (fsfy::Running, Client) {
    let running = fsfy::start(
        fsfy::Options { data: dir.join("data"), port: Some(0), window_port: Some(0) },
        Arc::new(fsfy::host::Headless),
    )
    .await
    .unwrap();
    let mut host = Client::new(format!("http://127.0.0.1:{}", running.app.ports.1));
    let r = host.http.get(running.entry_url()).send().await.unwrap();
    host.take_cookie(&r);
    assert!(host.send("PUT", "/api/admin/settings", json!({"provider": "off"})).await.status().is_success());
    (running, host)
}

/// Ждать, пока условие не выполнится (фоновые задачи), не дольше 30 с.
pub async fn until<F, Fut>(mut f: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for _ in 0..300 {
        if f().await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    false
}
