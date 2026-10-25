mod common;
use rusqlite::params;
use serde_json::{json, Value};

#[tokio::test(flavor = "multi_thread")]
async fn host_reset_without_email_revokes_sessions_and_old_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let mut user = common::Client::new(format!("http://127.0.0.1:{}", running.app.ports.0));
    assert!(user
        .send("POST", "/api/auth/register", json!({"login":"noemail","name":"Без почты","password":"OldPass123"}))
        .await
        .status()
        .is_success());
    let id = user.json("/api/me").await["user"]["id"].as_i64().unwrap();
    assert_eq!(
        user.send("PATCH", &format!("/api/admin/users/{id}"), json!({"password":"IllegalPass123"})).await.status(),
        403
    );
    running
        .app
        .db
        .with(|c| {
            c.execute(
                "INSERT INTO account_tokens VALUES(?,?,?,?,?,?)",
                params![
                    "obsolete",
                    id,
                    "reset",
                    "unused@example.com",
                    fsfy::util::now_ms() + 900000,
                    fsfy::util::now_ms()
                ],
            )
        })
        .unwrap();
    assert!(host
        .send("PATCH", &format!("/api/admin/users/{id}"), json!({"password":"HostReset123"}))
        .await
        .status()
        .is_success());
    assert_eq!(user.get("/api/me").await.status(), 401);
    let count: i64 = running
        .app
        .db
        .with(|c| c.query_row("SELECT count(*) FROM account_tokens WHERE user_id=?", [id], |r| r.get(0)))
        .unwrap();
    assert_eq!(count, 0);
    assert!(!user
        .send("POST", "/api/auth/login", json!({"login":"noemail","password":"OldPass123"}))
        .await
        .status()
        .is_success());
    assert!(user
        .send("POST", "/api/auth/login", json!({"login":"noemail","password":"HostReset123"}))
        .await
        .status()
        .is_success());
}

#[tokio::test(flavor = "multi_thread")]
async fn verified_email_reset_single_use_expiry_and_session_revocation() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let app = &running.app;
    let mut user = common::Client::new(format!("http://127.0.0.1:{}", app.ports.0));
    assert!(user
        .send("POST", "/api/auth/register", json!({"login":"mailviewer","name":"Почта","password":"OldPass123"}))
        .await
        .status()
        .is_success());
    let id = user.json("/api/me").await["user"]["id"].as_i64().unwrap();
    let issue = |purpose: &str, email: &str, expired: bool| {
        let raw = fsfy::util::token(32);
        app.db
            .with(|c| {
                c.execute(
                    "INSERT INTO account_tokens VALUES(?,?,?,?,?,?)",
                    params![
                        fsfy::util::sha256_hex(raw.as_bytes()),
                        id,
                        purpose,
                        email,
                        fsfy::util::now_ms() + if expired { -1000 } else { 900000 },
                        fsfy::util::now_ms()
                    ],
                )
            })
            .unwrap();
        raw
    };
    let raw = issue("verify", "viewer@example.com", false);
    assert!(user.send("POST", "/api/auth/verify-email", json!({"token":raw})).await.status().is_success());
    assert_eq!(user.json("/api/me/email").await["email"], "viewer@example.com");
    assert_eq!(user.send("POST", "/api/auth/verify-email", json!({"token":raw})).await.status(), 400);
    let known: Value = user
        .send("POST", "/api/auth/forgot-password", json!({"email":"viewer@example.com"}))
        .await
        .json()
        .await
        .unwrap();
    let unknown: Value = user
        .send("POST", "/api/auth/forgot-password", json!({"email":"missing@example.com"}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(known, unknown, "ответ не раскрывает наличие аккаунта");
    let expired = issue("reset", "viewer@example.com", true);
    assert_eq!(
        user.send("POST", "/api/auth/reset-password", json!({"token":expired,"newPassword":"NewPass123"}))
            .await
            .status(),
        400
    );
    let verify = issue("verify", "other@example.com", false);
    assert_eq!(
        user.send("POST", "/api/auth/reset-password", json!({"token":verify,"newPassword":"NewPass123"}))
            .await
            .status(),
        400,
        "назначение токена проверяется"
    );
    let valid = issue("reset", "viewer@example.com", false);
    let stored: String = app
        .db
        .with(|c| {
            c.query_row(
                "SELECT hash FROM account_tokens WHERE hash=?",
                [fsfy::util::sha256_hex(valid.as_bytes())],
                |r| r.get(0),
            )
        })
        .unwrap();
    assert_ne!(stored, valid);
    assert!(user
        .send("POST", "/api/auth/reset-password", json!({"token":valid,"newPassword":"NewPass123"}))
        .await
        .status()
        .is_success());
    assert_eq!(user.get("/api/me").await.status(), 401, "старый сеанс закрыт");
    assert_eq!(
        user.send("POST", "/api/auth/reset-password", json!({"token":valid,"newPassword":"AnotherPass123"}))
            .await
            .status(),
        400,
        "повтор запрещён"
    );
    assert_eq!(
        user.send("POST", "/api/auth/login", json!({"login":"mailviewer","password":"OldPass123"})).await.status(),
        401
    );
    assert!(user
        .send("POST", "/api/auth/login", json!({"login":"mailviewer","password":"NewPass123"}))
        .await
        .status()
        .is_success());
    assert!(!user.get("/api/admin/mail").await.status().is_success());
    assert!(host.send("PUT","/api/admin/mail",json!({"enabled":false,"host":"smtp.gmail.com","port":465,"tls":"tls","user":"owner@gmail.com","from":"","base":"","password":"test mail secret"})).await.status().is_success());
    let settings = host.json("/api/admin/mail").await;
    assert_eq!(settings["passwordSet"], true);
    assert!(settings.get("password").is_none());
    assert!(!settings.to_string().contains("testmailsecret"));
    assert!(!user
        .send("POST", "/api/me/email", json!({"email":"new@example.com","password":"NewPass123"}))
        .await
        .status()
        .is_success());
    assert_eq!(
        user.json("/api/me/email").await["email"],
        "viewer@example.com",
        "неудачная отправка не теряет подтверждённую почту"
    );
    running.shutdown().await;
}
