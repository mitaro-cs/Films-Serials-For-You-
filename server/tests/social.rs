//! Люди: регистрация без приглашений, отзывы с оценками и лайками, друзья, переписка с
//! фильмом, фото профиля, детский профиль без общения, пределы.

mod common;

use common::{ffmpeg, make_video, until, Client};
use serde_json::{json, Value};

async fn signup(public: &str, login: &str) -> Client {
    let mut c = Client::new(public.to_string());
    let r =
        c.send("POST", "/api/auth/register", json!({"name": login, "login": login, "password": "пароль-12345"})).await;
    assert!(r.status().is_success(), "регистрация {login}: {}", r.status());
    c
}

#[tokio::test(flavor = "multi_thread")]
async fn reviews_friends_chat_photo() {
    if !ffmpeg() {
        eprintln!("нет ffmpeg - пропуск");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let media = dir.path().join("media");
    make_video(&media.join("Дюна (2021)/dune.mp4"), "aac");
    let (running, mut host) = common::start(dir.path()).await;
    let public = format!("http://127.0.0.1:{}", running.app.ports.0);
    host.send("POST", "/api/admin/libraries", json!({"path": media.to_str().unwrap()})).await;
    assert!(until(|| async { host.json("/api/titles").await.as_array().unwrap().len() == 1 }).await);
    let dune = host.json("/api/titles").await[0]["id"].as_i64().unwrap();

    // Регистрация открыта по умолчанию: без хоста и без ссылок.
    assert_eq!(host.json("/api/admin/settings").await["registration"], "open");
    let mut anna = signup(&public, "anna").await;
    let mut boris = signup(&public, "boris").await;
    let pa = anna.json("/api/me").await["user"]["profile"]["id"].as_i64().unwrap();
    let pb = boris.json("/api/me").await["user"]["profile"]["id"].as_i64().unwrap();

    // ---------- отзывы ----------
    let r: Value = anna
        .send("POST", &format!("/api/titles/{dune}/reviews"), json!({"stars": 9, "body": "  Красиво!  "}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(r["body"], "Красиво!");
    assert_eq!(r["author"]["name"], "anna");
    let rid = r["id"].as_i64().unwrap();
    boris.send("POST", &format!("/api/titles/{dune}/reviews"), json!({"stars": 6, "body": "Долго"})).await;
    // Только комментарий, без оценки - в среднюю не идёт.
    boris.send("POST", &format!("/api/titles/{dune}/reviews"), json!({"body": "Но музыка ❤"})).await;
    let list = anna.json(&format!("/api/titles/{dune}/reviews")).await;
    assert_eq!(list["total"], 3);
    assert_eq!(list["votes"], 2);
    assert_eq!(list["average"], 3.75, "(4,5 + 3) / 2");
    assert_eq!(list["items"][0]["body"], "Но музыка ❤", "новые сверху");

    let bad = anna.send("POST", &format!("/api/titles/{dune}/reviews"), json!({"stars": 11})).await;
    assert_eq!(bad.status(), 400);
    let empty = anna.send("POST", &format!("/api/titles/{dune}/reviews"), json!({"body": "   "})).await;
    assert_eq!(empty.status(), 400);
    let long = anna.send("POST", &format!("/api/titles/{dune}/reviews"), json!({"body": "я".repeat(4001)})).await;
    assert_eq!(long.status(), 400);

    // Лайк, правка своего, чужое не править и не удалять, хост - удаляет.
    let l: Value =
        boris.send("PUT", &format!("/api/reviews/{rid}/like"), json!({"value": true})).await.json().await.unwrap();
    assert_eq!(l["likes"], 1);
    assert_eq!(boris.send("PATCH", &format!("/api/reviews/{rid}"), json!({"body": "взлом"})).await.status(), 403);
    assert_eq!(boris.send("DELETE", &format!("/api/reviews/{rid}"), json!({})).await.status(), 403);
    let e: Value = anna
        .send("PATCH", &format!("/api/reviews/{rid}"), json!({"stars": 10, "body": "Шедевр"}))
        .await
        .json()
        .await
        .unwrap();
    assert!(e["editedAt"].as_i64().is_some());
    assert_eq!(e["likes"], 1);
    let boris_comment = list["items"][0]["id"].as_i64().unwrap();
    assert!(host.send("DELETE", &format!("/api/reviews/{boris_comment}"), json!({})).await.status().is_success());
    assert_eq!(anna.json(&format!("/api/titles/{dune}/reviews")).await["total"], 2);

    // Убрать свою оценку: отзыв из одних звёзд исчезает, у отзыва с текстом остаётся текст.
    boris.send("POST", &format!("/api/titles/{dune}/reviews"), json!({"stars": 8})).await;
    assert_eq!(anna.json(&format!("/api/titles/{dune}/reviews")).await["votes"], 2);
    assert!(boris.send("DELETE", &format!("/api/titles/{dune}/stars"), json!({})).await.status().is_success());
    let list = anna.json(&format!("/api/titles/{dune}/reviews")).await;
    assert_eq!((list["total"].as_i64(), list["votes"].as_i64()), (Some(2), Some(1)));
    let long = list["items"].as_array().unwrap().iter().find(|r| r["body"] == "Долго").unwrap();
    assert_eq!(long["stars"], Value::Null);

    // ---------- участники и друзья ----------
    let m = anna.json("/api/members").await;
    let b = m.as_array().unwrap().iter().find(|x| x["id"] == pb).unwrap();
    assert_eq!(b["relation"], "none");
    assert_eq!(b["reviews"], 1);
    running
        .app
        .db
        .with(|c| {
            c.execute(
                "INSERT INTO ext_titles(id,kind,title,updated_at) VALUES('tt9999001','movie','Внешний фильм',0)",
                [],
            )?;
            c.execute("INSERT INTO ext_marks(profile_id,ext_id,status,at) VALUES(?,'tt9999001','watched',0)", [pb])?;
            Ok(())
        })
        .unwrap();
    let members = anna.json("/api/members").await;
    assert_eq!(members.as_array().unwrap().iter().find(|x| x["id"] == pb).unwrap()["watched"], 1);
    running.app.db.with(|c| {
        c.execute("INSERT INTO titles(kind,key,title,imdb_id,added_at,updated_at) VALUES('movie','same-external','Внешний фильм','tt9999001',0,0)", [])?;
        c.execute("INSERT INTO watched(profile_id,title_id,at) VALUES(?,?,0)", rusqlite::params![pb,c.last_insert_rowid()])?;
        Ok(())
    }).unwrap();
    let members = anna.json("/api/members").await;
    assert_eq!(
        members.as_array().unwrap().iter().find(|x| x["id"] == pb).unwrap()["watched"],
        1,
        "Один фильм из двух источников считается один раз"
    );
    // Писать можно только друзьям.
    assert_eq!(anna.send("POST", &format!("/api/chats/{pb}"), json!({"body": "привет"})).await.status(), 403);
    let r: Value = anna.send("POST", &format!("/api/friends/{pb}"), json!({})).await.json().await.unwrap();
    assert_eq!(r["relation"], "outgoing");
    assert_eq!(boris.json("/api/badges").await["requests"], 1);
    assert_eq!(boris.json("/api/members").await[0]["relation"], "incoming", "просьбы - сверху");
    let r: Value = boris.send("POST", &format!("/api/friends/{pa}"), json!({})).await.json().await.unwrap();
    assert_eq!(r["relation"], "friend");
    assert_eq!(anna.send("POST", &format!("/api/friends/{pa}"), json!({})).await.status(), 400, "себе - нельзя");

    // ---------- переписка ----------
    let s: Value = anna
        .send("POST", &format!("/api/chats/{pb}"), json!({"body": "Посмотри!", "titleId": dune}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(s["title"]["title"], "Дюна");
    assert_eq!(boris.json("/api/badges").await["unread"], 1);
    let c = boris.json("/api/chats").await;
    assert_eq!(c["items"][0]["person"]["id"], pa);
    assert_eq!(c["items"][0]["unread"], 1);
    let chat = boris.json(&format!("/api/chats/{pa}")).await;
    assert_eq!(chat["items"][0]["body"], "Посмотри!");
    assert_eq!(chat["items"][0]["mine"], false);
    assert_eq!(boris.json("/api/badges").await["unread"], 0, "открыли - прочитано");
    let last = s["id"].as_i64().unwrap();
    boris.send("POST", &format!("/api/chats/{pa}"), json!({"body": "Уже смотрю"})).await;
    let new = anna.json(&format!("/api/chats/{pb}?after={last}")).await;
    assert_eq!(new["items"].as_array().unwrap().len(), 1, "опрос отдаёт только новое");
    assert_eq!(new["items"][0]["body"], "Уже смотрю");
    assert_eq!(anna.send("POST", &format!("/api/chats/{pb}"), json!({"body": "  "})).await.status(), 400);

    // ---------- совместный список ----------
    let l: Value = anna
        .send("POST", "/api/lists", json!({"name": "  Смотрим вместе ", "members": [pb]}))
        .await
        .json()
        .await
        .unwrap();
    let lid = l["id"].as_i64().unwrap();
    let r = anna.send("PUT", &format!("/api/lists/{lid}/items/{dune}"), json!({"value": true})).await;
    assert!(r.status().is_success());
    let mine = boris.json("/api/lists").await;
    assert_eq!((mine[0]["name"].as_str(), mine[0]["count"].as_i64()), (Some("Смотрим вместе"), Some(1)));
    assert_eq!(mine[0]["members"].as_array().unwrap().len(), 2);
    assert_eq!(mine[0]["mine"], false);
    boris.send("PUT", &format!("/api/lists/{lid}/items/{dune}/watched"), json!({"value": true})).await;
    let full = anna.json(&format!("/api/lists/{lid}")).await;
    assert_eq!(full["items"][0]["card"]["title"], "Дюна");
    assert_eq!(full["items"][0]["addedBy"]["id"], pa);
    assert!(full["items"][0]["watchedAt"].as_i64().is_some(), "посмотрели вместе - видно всем");
    assert_eq!(anna.json(&format!("/api/titles/{dune}/lists")).await[0]["has"], true);
    assert_eq!(host.get(&format!("/api/lists/{lid}")).await.status(), 404, "чужой список не виден");
    assert_eq!(
        host.send("POST", "/api/lists", json!({"name": "x", "members": [pa]})).await.status(),
        403,
        "позвать можно только друга"
    );
    let left: Value = boris.send("DELETE", &format!("/api/lists/{lid}"), json!({})).await.json().await.unwrap();
    assert_eq!(left["left"], true, "участник выходит, список остаётся");
    assert_eq!(anna.json("/api/lists").await.as_array().unwrap().len(), 1);

    // Убрали из друзей - переписка закрыта.
    boris.send("DELETE", &format!("/api/friends/{pa}"), json!({})).await;
    assert_eq!(anna.send("POST", &format!("/api/chats/{pb}"), json!({"body": "эй"})).await.status(), 403);

    // ---------- страница участника ----------
    let page = boris.json(&format!("/api/members/{pa}")).await;
    assert_eq!(page["person"]["name"], "anna");
    assert_eq!(page["reviews"][0]["title"]["title"], "Дюна");
    assert_eq!(page["relation"], "none");

    // ---------- фото профиля ----------
    let jpg = std::fs::read(dir.path().join("p.jpg")).unwrap_or_else(|_| {
        assert!(std::process::Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=red:s=640x480", "-frames:v", "1"])
            .arg(dir.path().join("p.jpg"))
            .status()
            .unwrap()
            .success());
        std::fs::read(dir.path().join("p.jpg")).unwrap()
    });
    let r = anna
        .http
        .put(format!("{public}/api/profiles/{pa}/photo"))
        .header("cookie", &anna.cookie)
        .header("x-fsfy", "1")
        .body(jpg.clone())
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let photo = r.json::<Value>().await.unwrap()["photo"].as_str().unwrap().to_string();
    let img = anna.get(&photo).await.bytes().await.unwrap();
    assert!(img.len() < jpg.len() * 4, "уменьшено");
    assert_eq!(anna.json("/api/me").await["user"]["profile"]["photo"], photo);
    // Чужой профиль - нельзя; не картинка - нельзя.
    let r = boris
        .http
        .put(format!("{public}/api/profiles/{pa}/photo"))
        .header("cookie", &boris.cookie)
        .header("x-fsfy", "1")
        .body(jpg)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
    let r = anna
        .http
        .put(format!("{public}/api/profiles/{pa}/photo"))
        .header("cookie", &anna.cookie)
        .header("x-fsfy", "1")
        .body("<svg/>")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);

    // ---------- регистрация: хост закрывает и открывает ----------
    let mut extra = Client::new(public.clone());
    host.send("PUT", "/api/admin/settings", json!({"registration": "invite"})).await;
    let r =
        extra.send("POST", "/api/auth/register", json!({"name": "x", "login": "x1", "password": "пароль-12345"})).await;
    assert_eq!(r.status(), 400, "без приглашения - нельзя");
    host.send("PUT", "/api/admin/settings", json!({"registration": "open"})).await;

    // Открытая регистрация: не больше 5 аккаунтов с адреса за 10 минут (двое уже есть).
    for n in 0..3 {
        signup(&public, &format!("bot{n}")).await;
    }
    let r = extra
        .send("POST", "/api/auth/register", json!({"name": "x", "login": "bot9", "password": "пароль-12345"}))
        .await;
    assert_eq!(r.status(), 429);
    drop(running);
}
