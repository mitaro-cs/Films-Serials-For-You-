//! Добавление фильмов: своя папка, загрузка из браузера, слежение за папками (без кнопки
//! «Обновить»), перетаскивание папок и файлов в окно.

mod common;

use common::{ffmpeg, make_video, until};
use serde_json::{json, Value};

async fn titles(host: &common::Client) -> Vec<String> {
    host.json("/api/titles")
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn upload_watch_and_drop() {
    if !ffmpeg() {
        eprintln!("нет ffmpeg — пропуск");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;

    // Своя папка создаётся по просьбе и сразу подключается.
    let r: Value = host.send("POST", "/api/admin/inbox", json!({})).await.json().await.unwrap();
    let inbox = std::path::PathBuf::from(r["path"].as_str().unwrap());
    assert!(inbox.is_dir());
    assert_eq!(host.json("/api/admin/libraries").await.as_array().unwrap().len(), 1);
    assert_eq!(host.json("/api/admin/status").await["inbox"], inbox.to_str().unwrap());

    // Загрузка из браузера: потоком, в подпапку; плохие имена и типы — нет.
    let src = dir.path().join("src");
    make_video(&src.join("Загрузка.mp4"), "aac");
    let bytes = std::fs::read(src.join("Загрузка.mp4")).unwrap();
    let up = |name: &str, folder: &str, body: Vec<u8>| {
        host.http
            .put(format!("{}/api/admin/upload", host.base))
            .query(&[("name", name), ("folder", folder)])
            .header("cookie", &host.cookie)
            .header("x-fsfy", "1")
            .body(body)
            .send()
    };
    let r = up("Загрузка.mp4", "Загрузка (2024)", bytes.clone()).await.unwrap();
    assert!(r.status().is_success(), "{}", r.status());
    assert!(inbox.join("Загрузка (2024)/Загрузка.mp4").is_file());
    assert!(until(|| async { titles(&host).await.contains(&"Загрузка".to_string()) }).await);
    assert_eq!(up("Загрузка.mp4", "Загрузка (2024)", bytes.clone()).await.unwrap().status(), 400, "уже есть");
    assert_eq!(up("virus.exe", "", bytes.clone()).await.unwrap().status(), 400);
    assert_eq!(up("пусто.mp4", "", vec![]).await.unwrap().status(), 400);
    assert!(!inbox.join("пусто.mp4.part").exists(), "недогруженное убрано");
    // За пределы своей папки не выйти: «..» в папке — отказ, путь в имени — только имя.
    assert_eq!(up("escape.mp4", "../..", bytes.clone()).await.unwrap().status(), 400);
    assert!(up("../../escape.mp4", "", bytes.clone()).await.unwrap().status().is_success());
    assert!(inbox.join("escape.mp4").is_file());
    assert!(!dir.path().join("escape.mp4").exists());
    assert_eq!(up("a.mp4", "Сериал/../../x", bytes.clone()).await.unwrap().status(), 400);
    // Сериал папкой: структура сохраняется, серии без сезона — один сериал.
    for n in ["01. Пилот.mp4", "02. Второй день.mp4"] {
        let r = up(n, "Мой сериал/Сезон 1", bytes.clone()).await.unwrap();
        assert!(r.status().is_success(), "{}", r.status());
    }
    assert!(inbox.join("Мой сериал/Сезон 1/02. Второй день.mp4").is_file());
    assert!(until(|| async { titles(&host).await.contains(&"Мой сериал".to_string()) }).await);
    // Зритель загружать не может.
    let mut viewer = common::Client::new(format!("http://127.0.0.1:{}", running.app.ports.0));
    viewer
        .send("POST", "/api/auth/register", json!({"name": "v", "login": "viewer1", "password": "пароль-12345"}))
        .await;
    let r = viewer
        .http
        .put(format!("{}/api/admin/upload?name=x.mp4", viewer.base))
        .header("cookie", &viewer.cookie)
        .header("x-fsfy", "1")
        .body(bytes.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);

    // Слежение: положили файл в папку — появился сам, без «Обновить».
    make_video(&inbox.join("Сам (2023)/sam.mp4"), "aac");
    assert!(until(|| async { titles(&host).await.contains(&"Сам".to_string()) }).await, "слежение за папкой");

    // Перетаскивание: папка подключается как есть, файл копируется в свою папку.
    let elsewhere = dir.path().join("elsewhere");
    make_video(&elsewhere.join("Коллекция/Старое (1999)/old.mp4"), "aac");
    make_video(&elsewhere.join("Один (2010).mp4"), "aac");
    std::fs::write(elsewhere.join("readme.txt"), "x").unwrap();
    let d = fsfy::library::inbox::import(
        &running.app,
        vec![
            elsewhere.join("Коллекция"),
            elsewhere.join("Один (2010).mp4"),
            elsewhere.join("readme.txt"),
            inbox.clone(),
        ],
    );
    assert_eq!(d.folders.len(), 1);
    assert_eq!(d.files, 1);
    assert_eq!(d.skipped, 2, "не видео и уже подключённое");
    assert!(
        until(|| async {
            let t = titles(&host).await;
            t.contains(&"Старое".to_string()) && t.contains(&"Один".to_string())
        })
        .await
    );
    assert!(inbox.join("Один (2010).mp4").is_file());
    assert!(elsewhere.join("Один (2010).mp4").is_file(), "оригинал на месте");
    let st = host.json("/api/admin/status").await;
    assert_eq!(st["import"]["running"], false);
    assert_eq!(st["import"]["filesDone"], 1);
    assert!(std::fs::read_dir(&inbox).unwrap().all(|e| !e.unwrap().path().to_string_lossy().ends_with(".part")));
    drop(running);
}
