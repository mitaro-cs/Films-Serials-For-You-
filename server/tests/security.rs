//! Alpha 0.4: вход в два шага, журнал входов, «Управление» только из окна хоста, фильм из
//! ленты в сообщении, ничего лишнего о хосте зрителям.

mod common;

use common::Client;
use serde_json::{json, Value};

async fn signup(public: &str, login: &str) -> Client {
    let mut c = Client::new(public.to_string());
    let r =
        c.send("POST", "/api/auth/register", json!({"name": login, "login": login, "password": "пароль-12345"})).await;
    assert!(r.status().is_success(), "регистрация {login}: {}", r.status());
    c
}

fn now() -> i64 {
    fsfy::util::now_ms()
}

#[tokio::test(flavor = "multi_thread")]
async fn two_step_login_journal_and_local_management() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let public = format!("http://127.0.0.1:{}", running.app.ports.0);
    let mut anna = signup(&public, "anna").await;

    // ---------- вход в два шага ----------
    let setup: Value = anna.send("POST", "/api/me/totp/setup", json!({})).await.json().await.unwrap();
    let secret = setup["secret"].as_str().unwrap().to_string();
    assert!(setup["uri"].as_str().unwrap().starts_with("otpauth://totp/Nova%3Aanna?secret="));
    let bad = anna.send("PUT", "/api/me/totp", json!({"code": "000000"})).await;
    assert_eq!(bad.status(), 400, "неверный код не включает");
    assert_eq!(anna.json("/api/me/security").await["totp"], false);
    let code = fsfy::totp::code(&secret, now()).unwrap();
    assert!(anna.send("PUT", "/api/me/totp", json!({"code": code})).await.status().is_success());
    assert_eq!(anna.json("/api/me/security").await["totp"], true);

    let mut phone = Client::new(public.clone());
    let r = phone.send("POST", "/api/auth/login", json!({"login": "anna", "password": "пароль-12345"})).await;
    assert_eq!(r.status(), 401);
    assert_eq!(r.json::<Value>().await.unwrap()["error"], "totp_required", "пароль верный - просим код");
    let r = phone
        .send("POST", "/api/auth/login", json!({"login": "anna", "password": "пароль-12345", "code": "123456"}))
        .await;
    assert_eq!(r.json::<Value>().await.unwrap()["error"], "bad_totp");
    let r = phone.send("POST", "/api/auth/login", json!({"login": "anna", "password": "чужой", "code": code})).await;
    assert_eq!(r.json::<Value>().await.unwrap()["error"], "bad_credentials", "код без пароля не пускает");
    let code = fsfy::totp::code(&secret, now()).unwrap();
    let r =
        phone.send("POST", "/api/auth/login", json!({"login": "anna", "password": "пароль-12345", "code": code})).await;
    assert!(r.status().is_success(), "{}", r.status());
    assert_eq!(phone.get("/api/me").await.status(), 200);

    // Журнал: удачные и неудачные входы, без адресов.
    let log = anna.json("/api/me/security").await["logins"].as_array().unwrap().clone();
    assert!(log.iter().any(|l| l["ok"] == true) && log.iter().any(|l| l["ok"] == false));
    assert!(log.iter().all(|l| l.get("ip").is_none()));
    let all = host.json("/api/admin/logins").await;
    assert!(all["items"].as_array().unwrap().iter().any(|l| l["login"] == "anna"));

    // Выключить - только с кодом.
    assert_eq!(anna.send("DELETE", "/api/me/totp", json!({"code": "000000"})).await.status(), 400);
    let code = fsfy::totp::code(&secret, now()).unwrap();
    assert!(anna.send("DELETE", "/api/me/totp", json!({"code": code})).await.status().is_success());
    assert_eq!(anna.json("/api/me/security").await["totp"], false);

    // ---------- «Управление» - только в окне хоста ----------
    let ida = anna.json("/api/me").await["user"]["id"].as_i64().unwrap();
    assert!(host
        .send("PATCH", &format!("/api/admin/users/{ida}"), json!({"role": "admin"}))
        .await
        .status()
        .is_success());
    assert_eq!(anna.json("/api/me").await["manage"], false, "из интернета меню управления нет");
    let r = anna.get("/api/admin/settings").await;
    assert_eq!(r.status(), 403);
    assert_eq!(r.json::<Value>().await.unwrap()["error"], "manage_local");
    // Хост разрешил управление из интернета - но только с входом в два шага.
    assert!(host.send("PUT", "/api/admin/settings", json!({"manageRemote": true})).await.status().is_success());
    assert_eq!(anna.get("/api/admin/settings").await.status(), 403, "без второго шага - нет");
    let setup: Value = anna.send("POST", "/api/me/totp/setup", json!({})).await.json().await.unwrap();
    let secret = setup["secret"].as_str().unwrap().to_string();
    let code = fsfy::totp::code(&secret, now()).unwrap();
    assert!(anna.send("PUT", "/api/me/totp", json!({"code": code})).await.status().is_success());
    assert_eq!(anna.get("/api/admin/settings").await.status(), 200);
    assert_eq!(anna.json("/api/me").await["manage"], true);
    // Сам себе разрешить управление из интернета администратор не может.
    assert_eq!(anna.send("PUT", "/api/admin/settings", json!({"manageRemote": false})).await.status(), 403);
    drop(running);
}

#[tokio::test(flavor = "multi_thread")]
async fn film_from_discover_in_chat() {
    let dir = tempfile::tempdir().unwrap();
    let (running, _host) = common::start(dir.path()).await;
    let public = format!("http://127.0.0.1:{}", running.app.ports.0);
    fsfy::discover::upsert(
        &running.app,
        &[fsfy::discover::Ext {
            id: "tt0241527".into(),
            series: false,
            title: "Гарри Поттер и философский камень".into(),
            original: Some("Harry Potter and the Sorcerer's Stone".into()),
            year: Some(2001),
            rating: Some(7.7),
            votes: Some(900_000),
            genres: vec!["Fantasy".into()],
            poster_url: None,
        }],
    )
    .unwrap();
    let mut anna = signup(&public, "anna").await;
    let mut boris = signup(&public, "boris").await;
    let pa = anna.json("/api/me").await["user"]["profile"]["id"].as_i64().unwrap();
    let pb = boris.json("/api/me").await["user"]["profile"]["id"].as_i64().unwrap();
    anna.send("POST", &format!("/api/friends/{pb}"), json!({})).await;
    boris.send("POST", &format!("/api/friends/{pa}"), json!({})).await;

    let m: Value = anna
        .send("POST", &format!("/api/chats/{pb}"), json!({"body": "", "extId": "tt0241527"}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(m["ext"]["title"], "Гарри Поттер и философский камень");
    assert_eq!(m["ext"]["year"], 2001);
    let chat = boris.json(&format!("/api/chats/{pa}")).await;
    assert_eq!(chat["items"][0]["ext"]["id"], "tt0241527", "у друга - карточкой");
    assert_eq!(boris.json("/api/chats").await["items"][0]["last"], "Фильм", "в списке чатов - не пустая строка");
    // Неизвестный или чужой id - нет.
    let r = anna.send("POST", &format!("/api/chats/{pb}"), json!({"extId": "tt9999999"})).await;
    assert_eq!(r.status(), 404);
    let r = anna.send("POST", &format!("/api/chats/{pb}"), json!({"extId": "../../etc"})).await;
    assert_eq!(r.status(), 404);
    drop(running);
}

#[test]
fn genre_words_are_not_titles() {
    assert!(fsfy::discover::genre_query("фантастика").is_some());
    assert!(fsfy::discover::genre_query("гарри потеет").is_none());
}
