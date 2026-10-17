//! Каталог-билеты: вкладки, фильтры «Хочу / Смотрю / Просмотрено», «где остановился»,
//! белый билет с датой, цвет карточки по постеру.

mod common;

use common::{ffmpeg, make_video, until};
use serde_json::{json, Value};
use std::process::Command;

fn titles(v: &Value) -> Vec<String> {
    v["items"].as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap().to_string()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn library_statuses_resume_and_accent() {
    if !ffmpeg() {
        eprintln!("нет ffmpeg — пропуск");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let media = dir.path().join("media");
    make_video(&media.join("Красный (2020)/red.mp4"), "aac");
    // Красный постер — карточка должна стать красноватой.
    assert!(Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=0xC82818:s=300x450", "-frames:v", "1"])
        .arg(media.join("Красный (2020)/poster.jpg"))
        .status()
        .unwrap()
        .success());
    make_video(&media.join("Второй (2021)/second.mp4"), "aac");
    make_video(&media.join("Шоу/Season 1/Шоу.s01e01.mp4"), "aac");
    make_video(&media.join("Шоу/Season 1/Шоу.s01e02.mp4"), "aac");

    let (running, mut host) = common::start(dir.path()).await;
    host.send("POST", "/api/admin/libraries", json!({"path": media.to_str().unwrap()})).await;
    assert!(until(|| async { host.json("/api/titles").await.as_array().unwrap().len() == 3 }).await);
    assert!(until(|| async { host.json("/api/admin/status").await["unprobed"] == 0 }).await);

    let all = host.json("/api/library").await;
    assert_eq!(all["counts"], json!({"all": 3, "want": 0, "watching": 0, "watched": 0}));
    assert_eq!(titles(&host.json("/api/library?tab=movies").await).len(), 2);
    assert_eq!(titles(&host.json("/api/library?tab=series").await), ["Шоу"]);
    assert_eq!(titles(&host.json("/api/library?tab=new").await).len(), 3);

    // Цвет по постеру появляется фоном.
    assert!(
        until(|| async {
            let l = host.json("/api/library?tab=movies").await;
            let red = l["items"].as_array().unwrap().iter().find(|t| t["title"] == "Красный").cloned().unwrap();
            red["accent"].as_str().is_some_and(|a| {
                let ch = |i: usize| u8::from_str_radix(&a[i..i + 2], 16).unwrap();
                ch(1) > ch(3) && ch(1) > ch(5)
            })
        })
        .await,
        "у красного постера красноватая карточка"
    );

    let id = |l: &Value, t: &str| {
        l["items"].as_array().unwrap().iter().find(|x| x["title"] == t).unwrap()["id"].as_i64().unwrap()
    };
    let (red, second, show) = (id(&all, "Красный"), id(&all, "Второй"), id(&all, "Шоу"));

    // «Хочу посмотреть» — это «Мой список».
    host.send("PUT", &format!("/api/list/{second}"), json!({"value": true})).await;
    assert_eq!(titles(&host.json("/api/library?status=want").await), ["Второй"]);

    // Начали серию — «Смотрю», и видно, где остановились.
    let t = host.json(&format!("/api/titles/{show}")).await;
    let e1 = t["files"][0]["id"].as_i64().unwrap();
    let e2 = t["files"][1]["id"].as_i64().unwrap();
    host.send("PUT", &format!("/api/progress/{e1}"), json!({"position": 3.0, "duration": 10.0})).await;
    let w = host.json("/api/library?status=watching").await;
    assert_eq!(titles(&w), ["Шоу"]);
    assert_eq!(w["items"][0]["status"], "watching");
    assert_eq!(w["items"][0]["resume"]["fileId"], e1);
    assert_eq!(w["items"][0]["resume"]["position"], 3.0);
    assert!(w["items"][0]["resume"]["label"].as_str().unwrap().contains("1 серия"));

    // Досмотрели первую — «где остановился» переезжает на вторую серию.
    let r: Value = host
        .send("PUT", &format!("/api/progress/{e1}"), json!({"position": 9.9, "duration": 10.0}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(r["watched"], false, "сериал досмотрен не весь");
    let w = host.json("/api/library?status=watching").await;
    assert_eq!(w["items"][0]["resume"]["fileId"], e2);
    // Досмотрели последнюю — сериал сам становится «Просмотрено».
    let r: Value = host
        .send("PUT", &format!("/api/progress/{e2}"), json!({"position": 9.9, "duration": 10.0}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(r["watched"], true);

    // Вручную «Просмотрено» — белый билет с датой, из «Хочу» уходит.
    host.send("PUT", &format!("/api/titles/{second}/watched"), json!({"value": true})).await;
    let done = host.json("/api/library?status=watched").await;
    assert_eq!(titles(&done), ["Второй", "Шоу"], "сначала отмеченное позже");
    assert!(done["items"][0]["watchedAt"].as_i64().unwrap() > 0);
    assert_eq!(host.json("/api/library").await["counts"], json!({"all": 3, "want": 0, "watching": 0, "watched": 2}));

    // Снять отметку — и прогресс с начала.
    host.send("PUT", &format!("/api/titles/{show}/watched"), json!({"value": false})).await;
    let t = host.json(&format!("/api/titles/{show}")).await;
    assert!(t["card"]["status"].is_null());
    assert!(t["files"][0]["progress"].is_null());

    // Статус — свой у каждого профиля.
    let kid: Value = host.send("POST", "/api/profiles", json!({"name": "Второй профиль"})).await.json().await.unwrap();
    host.send("POST", &format!("/api/profiles/{}/select", kid["id"]), json!({})).await;
    assert_eq!(host.json("/api/library").await["counts"]["watched"], 0);
    assert_eq!(host.json(&format!("/api/titles/{red}")).await["card"]["status"], Value::Null);

    // Чужой id и неверное тело — ошибки, а не падение.
    assert_eq!(host.send("PUT", "/api/titles/999999/watched", json!({"value": true})).await.status(), 404);
    assert_eq!(host.send("PUT", &format!("/api/titles/{red}/watched"), json!({"v": 1})).await.status(), 422);
    drop(running);
}
