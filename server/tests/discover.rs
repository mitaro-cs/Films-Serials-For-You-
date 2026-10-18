//! Лента IMDb без сети: фильмы не с сервиса, «Буду смотреть», просьбы хосту, группы в списках,
//! ссылки на видео с других сайтов.

mod common;

use common::Client;
use serde_json::{json, Value};

fn ext(id: &str, title: &str, series: bool) -> fsfy::discover::Ext {
    fsfy::discover::Ext {
        id: id.into(),
        series,
        title: title.into(),
        original: None,
        year: Some(2020),
        rating: Some(8.1),
        votes: Some(1000),
        genres: vec!["Драма".into()],
        poster_url: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn feed_marks_wishes_groups_links() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let app = running.app.clone();
    let public = format!("http://127.0.0.1:{}", app.ports.0);
    fsfy::discover::upsert(
        &app,
        &[ext("tt0111161", "Побег из Шоушенка", false), ext("tt0903747", "Во все тяжкие", true)],
    )
    .unwrap();
    app.db
        .with(|c| {
            c.execute_batch(
                "INSERT INTO ext_charts VALUES ('top_movies', 0, 'tt0111161'), ('popular_series', 0, 'tt0903747');",
            )
        })
        .unwrap();

    let mut anna = Client::new(public.clone());
    let r = anna
        .send("POST", "/api/auth/register", json!({"name": "Анна", "login": "anna", "password": "пароль-12345"}))
        .await;
    assert!(r.status().is_success());

    // Стена постеров на входе — без входа и только постеры подборок IMDb.
    let guest = Client::new(public.clone());
    assert_eq!(guest.json("/api/site/wall").await, json!([]), "постеров ещё нет");
    let poster = app.images.save(b"\x89PNG\r\n\x1a\n-test-poster").unwrap();
    let other = app.images.save(b"\x89PNG\r\n\x1a\n-host-image").unwrap();
    app.db.with(|c| c.execute("UPDATE ext_titles SET poster = ? WHERE id = 'tt0111161'", [&poster])).unwrap();
    let wall = guest.json("/api/site/wall").await;
    assert_eq!(wall, json!([format!("/api/site/wall/{poster}")]));
    assert_eq!(guest.get(wall[0].as_str().unwrap()).await.status(), 200);
    assert_eq!(guest.get(&format!("/api/site/wall/{other}")).await.status(), 404, "чужие картинки — нет");
    assert_eq!(guest.get(&format!("/img/{poster}")).await.status(), 401, "/img — только после входа");

    // Лента: подборки по порядку.
    let feed = anna.json("/api/discover").await;
    let keys: Vec<&str> = feed["rows"].as_array().unwrap().iter().map(|r| r["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["popular_series", "top_movies"]);
    assert_eq!(feed["rows"][1]["items"][0]["title"], "Побег из Шоушенка");
    assert_eq!(feed["rows"][1]["items"][0]["local"], Value::Null, "на сервисе нет");

    // «Буду смотреть» и «Просмотрел».
    let r = anna.send("PUT", "/api/discover/title/tt0111161/mark", json!({"status": "want"})).await;
    assert!(r.status().is_success());
    anna.send("PUT", "/api/discover/title/tt0903747/mark", json!({"status": "watched"})).await;
    let marks = anna.json("/api/me/marks").await;
    assert_eq!(marks["want"][0]["ext"]["id"], "tt0111161");
    assert_eq!(marks["watched"][0]["ext"]["mark"], "watched");
    assert_eq!(anna.send("PUT", "/api/discover/title/tt0111161/mark", json!({"status": "bad"})).await.status(), 400);
    assert_eq!(anna.send("PUT", "/api/discover/title/tt9999999/mark", json!({"status": "want"})).await.status(), 404);

    // «Хочу на сервисе» — хост видит просьбу, значок, закрывает.
    anna.send("PUT", "/api/discover/title/tt0111161/wish", json!({"value": true})).await;
    let w = host.json("/api/admin/wishes").await;
    assert_eq!(
        (w[0]["card"]["id"].as_str(), w[0]["count"].as_i64(), w[0]["people"].as_str()),
        (Some("tt0111161"), Some(1), Some("Анна"))
    );
    assert_eq!(host.json("/api/badges").await["wishes"], 1);
    assert_eq!(anna.json("/api/badges").await["wishes"], 0, "зрителю — не показываем");
    assert_eq!(anna.get("/api/admin/wishes").await.status(), 403);
    host.send("DELETE", "/api/admin/wishes/tt0111161", json!({})).await;
    assert_eq!(host.json("/api/badges").await["wishes"], 0);

    // Свой список с группами и фильмом из ленты.
    let l: Value = anna.send("POST", "/api/lists", json!({"name": "Хоррор на Хэллоуин"})).await.json().await.unwrap();
    let lid = l["id"].as_i64().unwrap();
    assert!(anna
        .send("PUT", &format!("/api/lists/{lid}/ext/tt0111161"), json!({"value": true}))
        .await
        .status()
        .is_success());
    let g: Value =
        anna.send("POST", &format!("/api/lists/{lid}/groups"), json!({"name": "Сначала"})).await.json().await.unwrap();
    let gid = g["id"].as_i64().unwrap();
    let r = anna.send("PUT", &format!("/api/lists/{lid}/move"), json!({"extId": "tt0111161", "group": gid})).await;
    assert!(r.status().is_success());
    let full = anna.json(&format!("/api/lists/{lid}")).await;
    assert_eq!(full["groups"][0]["name"], "Сначала");
    assert_eq!(
        (full["items"][0]["ext"]["id"].as_str(), full["items"][0]["groupId"].as_i64()),
        (Some("tt0111161"), Some(gid))
    );
    assert_eq!(anna.json("/api/lists").await[0]["count"], 1);
    assert_eq!(anna.json("/api/discover/title/tt0111161/lists").await[0]["has"], true);
    anna.send("DELETE", &format!("/api/lists/{lid}/groups/{gid}"), json!({})).await;
    assert_eq!(
        anna.json(&format!("/api/lists/{lid}")).await["items"][0]["groupId"],
        Value::Null,
        "группа ушла, фильм остался"
    );
    assert_eq!(
        host.send("PUT", &format!("/api/lists/{lid}/ext/tt0111161"), json!({"value": false})).await.status(),
        404
    );

    // Ссылки: к фильму из ленты и сами по себе; чужую удалить нельзя.
    let r = anna
        .send(
            "POST",
            "/api/links",
            json!({"url": "https://vk.com/video-1_2", "extId": "tt0111161", "label": "Фильм в VK"}),
        )
        .await;
    assert!(r.status().is_success(), "{}", r.status());
    let link: Value = r.json().await.unwrap();
    let r = anna.send("POST", "/api/links", json!({"url": "https://example.com/page"})).await;
    assert_eq!(r.status(), 400);
    anna.send("POST", "/api/links", json!({"url": "https://cdn.example.com/trailer.mp4", "label": "Трейлер"})).await;
    let mine = anna.json("/api/links?extId=tt0111161").await;
    assert_eq!(
        (mine[0]["kind"].as_str(), mine[0]["embed"].as_str()),
        (Some("vk"), Some("https://vkvideo.ru/video_ext.php?oid=-1&id=2&hd=2"))
    );
    assert_eq!(anna.json("/api/links").await[0]["label"], "Трейлер");
    let detail = anna.json("/api/discover/title/tt0111161").await;
    assert_eq!(detail["links"][0]["label"], "Фильм в VK");

    // Своя страница фильма из ленты: кадр, актёры, кадры и похожие — из базы, без IMDb.
    let extra = json!({"tagline": "Страх держит в плену", "people": [{"name": "Тим Роббинс", "job": "actor",
        "role": "Энди", "photo": poster}], "gallery": [poster, ""], "similar": ["tt0903747", "tt0000404"],
        "countries": ["США"], "poster": poster});
    app.db
        .with(|c| {
            c.execute(
                "UPDATE ext_titles SET backdrop = ?, extra = ?, detailed = ? WHERE id = 'tt0111161'",
                rusqlite::params![poster, extra.to_string(), fsfy::discover::DETAILED],
            )
        })
        .unwrap();
    let page = anna.json("/api/discover/title/tt0111161").await;
    assert_eq!(page["backdrop"], format!("/img/{poster}"));
    assert_eq!(page["posterLarge"], format!("/img/{poster}"));
    assert_eq!(page["tagline"], "Страх держит в плену");
    assert_eq!(page["people"][0]["photo"], format!("/img/{poster}"));
    assert_eq!(page["gallery"], json!([format!("/img/{poster}")]), "пустые имена — мимо");
    assert_eq!(page["similar"].as_array().unwrap().len(), 1, "неизвестный id — мимо");
    assert_eq!(page["similar"][0]["title"], "Во все тяжкие");
    assert_eq!(page["countries"], json!(["США"]));
    let lid2 = link["id"].as_i64().unwrap();
    let mut boris = Client::new(public.clone());
    boris.send("POST", "/api/auth/register", json!({"name": "Б", "login": "boris", "password": "пароль-12345"})).await;
    assert_eq!(boris.send("DELETE", &format!("/api/links/{lid2}"), json!({})).await.status(), 403);
    assert!(host.send("DELETE", &format!("/api/links/{lid2}"), json!({})).await.status().is_success(), "хост может");

    // Оформление профиля и статистика.
    let r = anna
        .send(
            "PATCH",
            "/api/me/profile",
            json!({"bio": "  Люблю драмы ", "accent": "#FF8800", "favorites": ["x:tt0903747", "x:tt0903747", "zz"]}),
        )
        .await;
    assert!(r.status().is_success());
    assert_eq!(anna.send("PATCH", "/api/me/profile", json!({"accent": "red"})).await.status(), 400);
    let pa = anna.json("/api/me").await["user"]["profile"]["id"].as_i64().unwrap();
    let page = host.json(&format!("/api/members/{pa}")).await;
    assert_eq!(
        (page["decor"]["bio"].as_str(), page["decor"]["accent"].as_str()),
        (Some("Люблю драмы"), Some("#ff8800"))
    );
    assert_eq!(page["decor"]["favorites"].as_array().unwrap().len(), 1, "повтор и мусор отброшены");
    assert_eq!(page["decor"]["favorites"][0]["ext"]["title"], "Во все тяжкие");
    assert_eq!(page["stats"]["outside"], 1, "просмотренное не с сервиса считается");

    // Любимые — как на IMDb, не больше пяти; с карточки фильма — по одному.
    let r: Value =
        anna.send("PUT", "/api/me/favorites", json!({"key": "x:tt0111161", "value": true})).await.json().await.unwrap();
    assert_eq!(r["favorites"], json!(["x:tt0903747", "x:tt0111161"]));
    assert_eq!(anna.json("/api/me").await["favorites"], json!(["x:tt0903747", "x:tt0111161"]));
    for i in 0..3 {
        let r = anna.send("PUT", "/api/me/favorites", json!({"key": format!("t:{}", 100 + i), "value": true})).await;
        assert!(r.status().is_success());
    }
    let sixth = anna.send("PUT", "/api/me/favorites", json!({"key": "t:200", "value": true})).await;
    assert_eq!(sixth.status(), 400, "шестой — нельзя");
    let r: Value =
        anna.send("PUT", "/api/me/favorites", json!({"key": "t:100", "value": false})).await.json().await.unwrap();
    assert_eq!(r["favorites"].as_array().unwrap().len(), 4);
    assert_eq!(anna.send("PUT", "/api/me/favorites", json!({"key": "zz", "value": true})).await.status(), 400);

    // Любимые режиссёры и лента просмотренного на обложке.
    let r = anna
        .send("PATCH", "/api/me/profile", json!({"directors": [" Нолан ", "нолан", "Финчер"], "coverKind": "ribbon"}))
        .await;
    assert!(r.status().is_success());
    app.db.with(|c| c.execute("UPDATE ext_titles SET poster = ? WHERE id = 'tt0903747'", [&poster])).unwrap();
    let page = host.json(&format!("/api/members/{pa}")).await;
    assert_eq!(page["decor"]["directors"], json!(["Нолан", "Финчер"]));
    assert_eq!(page["decor"]["coverKind"], "ribbon");
    assert_eq!(page["decor"]["ribbon"], json!([format!("/img/{poster}")]));
    assert_eq!(anna.send("PATCH", "/api/me/profile", json!({"coverKind": "video"})).await.status(), 400);
    assert_eq!(page["stats"]["months"].as_array().unwrap().len(), 12);
    drop(running);
}
