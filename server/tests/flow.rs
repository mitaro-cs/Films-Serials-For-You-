//! Сквозной сценарий: хост входит из окна, добавляет папку, зритель регистрируется по
//! приглашению, смотрит (Range и перепаковка ffmpeg), прогресс попадает в «Продолжить».

use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

fn ffmpeg() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success())
}

fn make_video(path: &Path, audio: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let ok = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=duration=4:size=320x240:rate=25"])
        .args(["-f", "lavfi", "-i", "sine=duration=4", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", audio])
        .arg(path)
        .status()
        .unwrap()
        .success();
    assert!(ok, "ffmpeg не сделал тестовое видео");
}

struct Client {
    base: String,
    http: reqwest::Client,
    cookie: String,
}

impl Client {
    fn new(base: String) -> Self {
        Self {
            base,
            http: reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap(),
            cookie: String::new(),
        }
    }

    fn take_cookie(&mut self, res: &reqwest::Response) {
        if let Some(c) = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()) {
            self.cookie = c.split(';').next().unwrap().to_string();
        }
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.http.get(format!("{}{path}", self.base)).header("cookie", &self.cookie).send().await.unwrap()
    }

    async fn json(&self, path: &str) -> Value {
        let r = self.get(path).await;
        assert!(r.status().is_success(), "{path}: {}", r.status());
        r.json().await.unwrap()
    }

    async fn send(&mut self, method: &str, path: &str, body: Value) -> reqwest::Response {
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

#[tokio::test(flavor = "multi_thread")]
async fn host_adds_folder_and_viewer_watches() {
    if !ffmpeg() {
        eprintln!("нет ffmpeg — пропуск");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let media = dir.path().join("media");
    make_video(&media.join("Test Movie (2020)/Test.Movie.2020.1080p.mp4"), "aac");
    make_video(&media.join("Show/Season 1/Show.S01E01.mkv"), "ac3");
    make_video(&media.join("Show/Season 1/Show.S01E02.mkv"), "ac3");
    std::fs::write(
        media.join("Test Movie (2020)/Test.Movie.2020.1080p.ru.srt"),
        "1\n00:00:00,500 --> 00:00:02,000\nПривет\n",
    )
    .unwrap();

    let running = fsfy::start(
        fsfy::Options { data: dir.path().join("data"), port: Some(0), window_port: Some(0) },
        Arc::new(fsfy::host::Headless),
    )
    .await
    .unwrap();
    let (port, wport) = running.app.ports;

    // Хост: одноразовая ссылка окна.
    let mut host = Client::new(format!("http://127.0.0.1:{wport}"));
    let entry = running.entry_url();
    let r = host.http.get(&entry).send().await.unwrap();
    host.take_cookie(&r);
    assert!(host.cookie.starts_with("fsfy_w="));
    // Та же ссылка второй раз не входит, а cookie окна не действует на общем порту.
    let again = host.http.get(&entry).send().await.unwrap();
    assert!(again.headers().get("set-cookie").is_none());
    let mut wrong = Client::new(format!("http://127.0.0.1:{port}"));
    wrong.cookie = host.cookie.replace("fsfy_w", "fsfy_s");
    assert_eq!(wrong.get("/api/me").await.status(), 401);
    assert_eq!(host.json("/api/me").await["user"]["role"], "host");

    // Без заголовка X-Fsfy изменения не принимаются.
    let csrf = host
        .http
        .put(format!("{}/api/admin/settings", host.base))
        .header("cookie", &host.cookie)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(csrf.status(), 403);

    assert!(host
        .send("PUT", "/api/admin/settings", json!({"provider": "off", "registration": "invite"}))
        .await
        .status()
        .is_success());
    let r = host.send("POST", "/api/admin/libraries", json!({"path": media.to_str().unwrap()})).await;
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());

    // Обход и ffprobe — в фоне.
    let mut titles = Value::Null;
    for _ in 0..100 {
        titles = host.json("/api/titles").await;
        let probed = host.json("/api/admin/status").await["unprobed"] == 0;
        if titles.as_array().unwrap().len() == 2 && probed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let titles = titles.as_array().unwrap().clone();
    assert_eq!(titles.len(), 2, "{titles:?}");
    let movie = titles.iter().find(|t| t["kind"] == "movie").unwrap();
    assert_eq!((movie["title"].as_str(), movie["year"].as_i64()), (Some("Test Movie"), Some(2020)));
    let show = titles.iter().find(|t| t["kind"] == "series").unwrap();
    assert_eq!(show["files"], 2);

    // Приглашение и регистрация зрителя.
    let inv: Value = host.send("POST", "/api/admin/invites", json!({"uses": 1})).await.json().await.unwrap();
    let code = inv["code"].as_str().unwrap().to_string();
    let mut viewer = Client::new(format!("http://127.0.0.1:{port}"));
    assert_eq!(viewer.get("/api/home").await.status(), 401);
    let bad = viewer
        .send(
            "POST",
            "/api/auth/register",
            json!({"invite": "WRONG123", "name": "Аня", "login": "anya", "password": "secret1"}),
        )
        .await;
    assert_eq!(bad.status(), 400);
    let ok = viewer
        .send(
            "POST",
            "/api/auth/register",
            json!({"invite": code, "name": "Аня", "login": "anya", "password": "secret1"}),
        )
        .await;
    assert!(ok.status().is_success());
    let used = Client::new(viewer.base.clone())
        .send(
            "POST",
            "/api/auth/register",
            json!({"invite": code, "name": "Боря", "login": "borya", "password": "secret1"}),
        )
        .await;
    assert_eq!(used.status(), 400, "приглашение на одного");

    // Страница фильма: субтитры и прямой просмотр.
    let mid = movie["id"].as_i64().unwrap();
    let detail = viewer.json(&format!("/api/titles/{mid}")).await;
    let fid = detail["next"]["fileId"].as_i64().unwrap();
    let info = viewer.json(&format!("/api/files/{fid}/info")).await;
    assert_eq!(info["play"]["direct"], true);
    assert_eq!(info["play"]["subs"][0]["id"], "f0");
    let vtt = viewer.get(&format!("/api/files/{fid}/subs/f0")).await.text().await.unwrap();
    assert!(vtt.starts_with("WEBVTT") && vtt.contains("00:00:00.500"));

    let part = viewer
        .http
        .get(format!("{}/api/files/{fid}/raw", viewer.base))
        .header("cookie", &viewer.cookie)
        .header("range", "bytes=0-99")
        .send()
        .await
        .unwrap();
    assert_eq!(part.status(), 206);
    assert_eq!(part.bytes().await.unwrap().len(), 100);

    // Серия в MKV с AC3: только через перепаковку.
    let sid = show["id"].as_i64().unwrap();
    let sd = viewer.json(&format!("/api/titles/{sid}")).await;
    assert_eq!(sd["seasons"], json!([1]));
    let ep = sd["files"][0]["id"].as_i64().unwrap();
    let einfo = viewer.json(&format!("/api/files/{ep}/info")).await;
    assert_eq!(einfo["play"]["direct"], false);
    assert_eq!(einfo["next"], sd["files"][1]["id"]);
    let mp4 = viewer.get(&format!("/api/files/{ep}/play?t=1")).await;
    assert_eq!(mp4.headers()["content-type"], "video/mp4");
    let bytes = mp4.bytes().await.unwrap();
    assert!(bytes.len() > 1000 && &bytes[4..8] == b"ftyp", "фрагментированный MP4");

    // Досмотрел первую серию — «Продолжить» предлагает вторую.
    let r = viewer.send("PUT", &format!("/api/progress/{ep}"), json!({"position": 3.9, "duration": 4.0})).await;
    assert!(r.status().is_success());
    let home = viewer.json("/api/home").await;
    assert_eq!(home["continue"][0]["next"]["fileId"], sd["files"][1]["id"]);

    // Скачивание: запрет хоста действует.
    assert_eq!(viewer.get(&format!("/api/files/{fid}/download")).await.status(), 200);
    host.send("PUT", "/api/admin/settings", json!({"downloads": false})).await;
    assert_eq!(viewer.get(&format!("/api/files/{fid}/download")).await.status(), 403);

    // Зритель не управляет.
    assert_eq!(viewer.get("/api/admin/status").await.status(), 403);
    running.shutdown().await;
}
