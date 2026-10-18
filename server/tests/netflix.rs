//! Возможности в духе Netflix: профили и детский режим, оценки и «совпадение», Топ-10,
//! адаптивный HLS (проверяется настоящим ffmpeg-клиентом), кадры перемотки, заставка по
//! главам и отметкам хоста, лёгкие постеры.

mod common;

use common::{ffmpeg, make_video, make_video_with, until};
use serde_json::{json, Value};
use std::process::Command;

fn find<'a>(list: &'a Value, title: &str) -> &'a Value {
    list.as_array().unwrap().iter().find(|t| t["title"] == title).unwrap_or_else(|| panic!("нет «{title}» в {list}"))
}

fn ids(list: &Value) -> Vec<String> {
    list.as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap().to_string()).collect()
}

/// Время начала кусочка MPEG-TS по ffprobe.
fn start_time(bytes: &[u8], dir: &std::path::Path) -> f64 {
    let p = dir.join("seg.ts");
    std::fs::write(&p, bytes).unwrap();
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=start_time", "-of", "default=nw=1:nk=1"])
        .arg(&p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn profiles_taste_hls_trickplay_markers() {
    if !ffmpeg() {
        eprintln!("нет ffmpeg — пропуск");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let media = dir.path().join("media");
    let meta = dir.path().join("chapters.txt");
    std::fs::write(
        &meta,
        ";FFMETADATA1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=2000\ntitle=Пролог\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=2000\nEND=8000\ntitle=Opening\n\
         [CHAPTER]\nTIMEBASE=1/1000\nSTART=8000\nEND=16000\ntitle=Часть 1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=16000\nEND=20000\ntitle=Ending\n",
    )
    .unwrap();
    make_video_with(&media.join("Главы (2020)/glavy.2020.mp4"), "aac", 20, Some(&meta));
    let big_poster = media.join("Главы (2020)/poster.jpg");
    assert!(Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=1000x1500", "-frames:v", "1"])
        .arg(&big_poster)
        .status()
        .unwrap()
        .success());
    make_video(&media.join("Взрослый (2019)/adult.mp4"), "aac");
    make_video(&media.join("Мульт (2018)/cartoon.mp4"), "aac");
    make_video(&media.join("Комедия (2017)/comedy.mp4"), "aac");

    let (running, mut host) = common::start(dir.path()).await;
    host.send("POST", "/api/admin/libraries", json!({"path": media.to_str().unwrap()})).await;
    assert!(until(|| async { host.json("/api/titles").await.as_array().unwrap().len() == 4 }).await);
    let titles = host.json("/api/admin/titles").await;
    let id_of = |t: &str| find(&titles, t)["id"].as_i64().unwrap();
    let (glavy, adult, cartoon, comedy) = (id_of("Главы"), id_of("Взрослый"), id_of("Мульт"), id_of("Комедия"));
    host.send("PATCH", &format!("/api/admin/titles/{adult}"), json!({"age": "18+", "genres": ["Драма"]})).await;
    host.send("PATCH", &format!("/api/admin/titles/{cartoon}"), json!({"genres": ["Мультфильм"]})).await;
    host.send("PATCH", &format!("/api/admin/titles/{comedy}"), json!({"genres": ["Комедия"]})).await;
    host.send("PATCH", &format!("/api/admin/titles/{glavy}"), json!({"genres": ["Комедия", "Драма"]})).await;
    assert!(until(|| async { host.json("/api/admin/status").await["unprobed"] == 0 }).await);

    // ---------- один аккаунт — один профиль (выбора «Кто смотрит?» нет) ----------
    let me = host.json("/api/me").await;
    assert_eq!(me["user"]["profile"]["kids"], false);
    assert!(me.get("profiles").is_none() && me["user"].get("profileChosen").is_none());
    assert!(!host.send("POST", "/api/profiles", json!({"name": "Дети"})).await.status().is_success());
    assert_eq!(host.json("/api/titles").await.as_array().unwrap().len(), 4);
    host.send("PATCH", "/api/me", json!({"name": "Хост Кино", "avatar": 5, "autoplay": false})).await;
    let me = host.json("/api/me").await;
    assert_eq!(
        (me["user"]["name"].as_str(), me["user"]["profile"]["name"].as_str(), me["user"]["profile"]["avatar"].as_i64()),
        (Some("Хост Кино"), Some("Хост Кино"), Some(5))
    );
    assert_eq!(me["user"]["profile"]["autoplay"], false);

    // ---------- «Продолжить», Топ-10 ----------
    let file_of = |d: &Value| d["files"][0]["id"].as_i64().unwrap();
    let gd = host.json(&format!("/api/titles/{glavy}")).await;
    let glavy_file = file_of(&gd);
    for t in [comedy, cartoon] {
        let f = file_of(&host.json(&format!("/api/titles/{t}")).await);
        host.send("PUT", &format!("/api/progress/{f}"), json!({"position": 1.0, "duration": 4.0})).await;
    }
    host.send("PUT", &format!("/api/progress/{glavy_file}"), json!({"position": 5.0, "duration": 20.0})).await;
    let home = host.json("/api/home").await;
    assert_eq!(home["continue"][0]["id"], glavy);
    let top = home["rows"].as_array().unwrap().iter().find(|r| r["key"] == "top10").expect("Топ-10");
    assert_eq!(top["items"].as_array().unwrap().len(), 3);

    // ---------- оценки и «совпадение» ----------
    host.send("PUT", &format!("/api/titles/{comedy}/rating"), json!({"value": 2})).await;
    host.send("PUT", &format!("/api/titles/{adult}/rating"), json!({"value": -1})).await;
    let list = host.json("/api/titles").await;
    assert_eq!(find(&list, "Комедия")["myRating"], 2);
    assert_eq!(find(&list, "Комедия")["matchPct"], 99, "комедии по вкусу");
    assert!(find(&list, "Главы")["matchPct"].as_i64().is_none_or(|p| p < 99), "драма — «не для меня», совпадение ниже");
    let home = host.json("/api/home").await;
    let new_row = home["rows"].as_array().unwrap().iter().find(|r| r["key"] == "new").unwrap();
    assert!(!ids(&new_row["items"]).contains(&"Взрослый".to_string()), "«не для меня» не предлагаем");

    // ---------- заставка и титры ----------
    let info = host.json(&format!("/api/files/{glavy_file}/info")).await;
    assert_eq!(info["markers"]["intro"], json!([2.0, 8.0]));
    assert_eq!(info["markers"]["credits"], json!(16.0));
    assert_eq!(info["hls"], true);
    let r = host
        .send(
            "PUT",
            &format!("/api/files/{glavy_file}/markers"),
            json!({"introStart": 1.0, "introEnd": 5.0, "credits": 15.0}),
        )
        .await;
    assert!(r.status().is_success());
    let info = host.json(&format!("/api/files/{glavy_file}/info")).await;
    assert_eq!((info["markers"]["intro"].clone(), info["markers"]["manual"].clone()), (json!([1.0, 5.0]), json!(true)));
    let done: Value = host
        .send("PUT", &format!("/api/progress/{glavy_file}"), json!({"position": 15.5, "duration": 20.0}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(done["done"], true, "начались титры — досмотрено");

    // ---------- адаптивный HLS ----------
    let master = host.get(&format!("/api/files/{glavy_file}/hls/master.m3u8")).await.text().await.unwrap();
    assert!(master.contains("240/index.m3u8?audio=0"), "{master}");
    let variant = host.get(&format!("/api/files/{glavy_file}/hls/240/index.m3u8")).await.text().await.unwrap();
    assert_eq!(variant.matches("#EXTINF").count(), 4);
    // Перемотка: сразу третий кусочек, потом первый — время у кусочков то же, что в плейлисте.
    let seg2 = host.get(&format!("/api/files/{glavy_file}/hls/240/2.ts")).await;
    assert_eq!(seg2.status(), 200);
    let t2 = start_time(&seg2.bytes().await.unwrap(), dir.path());
    let seg0 = host.get(&format!("/api/files/{glavy_file}/hls/240/0.ts")).await.bytes().await.unwrap();
    let t0 = start_time(&seg0, dir.path());
    assert!(((t2 - t0) - 12.0).abs() < 0.3, "кусочек 2 начинается через 12 с после первого: {t0} → {t2}");
    // Настоящий HLS-клиент (ffmpeg) проигрывает весь поток.
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-headers", &format!("Cookie: {}\r\n", host.cookie)])
        .arg("-i")
        .arg(format!("{}/api/files/{glavy_file}/hls/master.m3u8", host.base))
        .args(["-f", "null", "-"])
        .output()
        .unwrap();
    assert!(out.status.success(), "ffmpeg: {}", String::from_utf8_lossy(&out.stderr));

    // ---------- кадры для перемотки и лёгкие постеры ----------
    assert!(
        until(|| async {
            host.json(&format!("/api/files/{glavy_file}/info")).await["trickplay"]["count"].as_i64() == Some(2)
        })
        .await
    );
    let sheet = host.get(&format!("/api/files/{glavy_file}/trickplay/0.jpg")).await.bytes().await.unwrap();
    assert_eq!(&sheet[..3], &[0xFF, 0xD8, 0xFF]);
    assert!(
        until(|| async {
            let c = &host.json(&format!("/api/titles/{glavy}")).await["card"];
            c["poster"].is_string() && c["poster"] != c["posterLarge"]
        })
        .await
    );

    running.shutdown().await;
}
