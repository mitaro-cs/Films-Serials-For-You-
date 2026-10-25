//! Скорость у зрителя: замер `/api/speedtest` (только для вошедших, несжатый, с пределом) и отчёт
//! плеера в прогрессе - хост видит в «Сейчас смотрят» скорость, качество и остановки.

mod common;

use common::{ffmpeg, make_video, until};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn speedtest_and_viewer_net_in_admin() {
    if !ffmpeg() {
        eprintln!("нет ffmpeg - пропуск");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let media = dir.path().join("media");
    make_video(&media.join("Фильм (2020)/film.mp4"), "aac");
    let (running, mut host) = common::start(dir.path()).await;

    // Без входа - нельзя.
    let anon = reqwest::get(format!("http://127.0.0.1:{}/api/speedtest", running.app.ports.0)).await.unwrap();
    assert_eq!(anon.status(), 401);

    // Сколько попросили (не больше 4 МБ), без сжатия и без кэша.
    let r = host
        .http
        .get(format!("{}/api/speedtest?bytes=200000", host.base))
        .header("cookie", &host.cookie)
        .header("accept-encoding", "gzip, br")
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    assert!(r.headers().get("content-encoding").is_none());
    assert_eq!(r.headers()["cache-control"], "no-store");
    assert_eq!(r.bytes().await.unwrap().len(), 200_000);
    let big = host.get("/api/speedtest?bytes=99999999").await;
    assert_eq!(big.bytes().await.unwrap().len(), 4_000_000);

    // Не чаще 20 раз в минуту.
    for _ in 0..18 {
        assert!(host.get("/api/speedtest?bytes=1000").await.status().is_success());
    }
    assert_eq!(host.get("/api/speedtest?bytes=1000").await.status(), 400);

    // Отчёт плеера вместе с местом просмотра - в «Сейчас смотрят».
    host.send("POST", "/api/admin/libraries", json!({"path": media.to_str().unwrap()})).await;
    assert!(until(|| async { host.json("/api/titles").await.as_array().unwrap().len() == 1 }).await);
    assert!(until(|| async { host.json("/api/admin/status").await["unprobed"] == 0 }).await);
    let title = host.json("/api/titles").await[0]["id"].as_i64().unwrap();
    let file = host.json(&format!("/api/titles/{title}")).await["files"][0]["id"].as_i64().unwrap();
    assert!(host.json(&format!("/api/files/{file}/info")).await["bitrate"].as_f64().is_some_and(|b| b > 0.0));
    let r = host
        .send(
            "PUT",
            &format!("/api/progress/{file}"),
            json!({"position": 2, "duration": 6, "net": {"mbit": 3.24, "quality": "720p", "stalls": 2}}),
        )
        .await;
    assert!(r.status().is_success());
    let w = &host.json("/api/admin/status").await["watching"][0];
    assert_eq!(w["net"]["mbit"], 3.2);
    assert_eq!(w["net"]["quality"], "720p");
    assert_eq!(w["net"]["stalls"], 2);

    // Чужое качество не сохраняется, а место просмотра - да.
    let r = host
        .send("PUT", &format!("/api/progress/{file}"), json!({"position": 3, "duration": 6, "net": {"quality": "<b>"}}))
        .await;
    assert!(r.status().is_success());
    assert_eq!(host.json("/api/admin/status").await["watching"][0]["net"]["quality"], "720p");
    drop(running);
}
