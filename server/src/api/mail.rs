use super::account::Ip;
use crate::{
    auth::{self, Manager, Viewer},
    error::{ApiError, ApiResult},
    util::{now_ms, sha256_hex, token},
    App,
};
use axum::{extract::State, Json};
use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

fn address(raw: &str) -> ApiResult<String> {
    let raw = raw.trim().to_lowercase();
    if raw.len() > 254
        || raw.chars().any(|c| c.is_control() || c.is_whitespace())
        || raw.parse::<lettre::Address>().is_err()
    {
        return Err(ApiError::bad("email", "Проверьте адрес электронной почты"));
    }
    Ok(raw)
}
pub(crate) fn ready(app: &App) -> bool {
    app.db.setting_or("mail.enabled", "0") == "1"
        && !app.db.setting_or("mail.user", "").is_empty()
        && !app.db.setting_or("mail.password", "").is_empty()
}
fn base(app: &App) -> String {
    let explicit = app.db.setting_or("mail.base", "");
    if !explicit.is_empty() {
        return explicit.trim_end_matches('/').into();
    }
    let view = app.access.view(app);
    view.public_url
        .or(view.lan_url)
        .unwrap_or_else(|| format!("http://127.0.0.1:{}", app.ports.0))
        .trim_end_matches('/')
        .into()
}
async fn send(app: &App, to: &str, subject: &str, body: String) -> ApiResult<()> {
    if !ready(app) {
        return Err(ApiError::bad("mail_disabled", "Владелец Nova ещё не настроил отправку писем"));
    }
    let host = app.db.setting_or("mail.host", "smtp.gmail.com");
    let user = app.db.setting_or("mail.user", "");
    let password = app.db.setting_or("mail.password", "");
    let from = app.db.setting_or("mail.from", &user);
    let message = Message::builder()
        .from(from.parse().map_err(|_| ApiError::bad("email", "Проверьте адрес отправителя"))?)
        .to(to.parse().map_err(|_| ApiError::bad("email", "Проверьте адрес получателя"))?)
        .subject(subject)
        .body(body)
        .map_err(ApiError::internal)?;
    let mode = app.db.setting_or("mail.tls", "tls");
    let builder = if mode == "starttls" {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
    }
    .map_err(|_| ApiError::bad("smtp", "Проверьте SMTP-сервер"))?;
    let transport = builder
        .port(app.db.setting_or("mail.port", "465").parse().unwrap_or(465))
        .credentials(Credentials::new(user, password))
        .timeout(Some(Duration::from_secs(15)))
        .build();
    tokio::time::timeout(Duration::from_secs(20), transport.send(message))
        .await
        .map_err(|_| ApiError::bad("smtp", "Почтовый сервер не ответил. Проверьте настройки"))?
        .map_err(|_| {
            ApiError::bad("smtp", "Не удалось отправить письмо. Проверьте Gmail, пароль приложения и соединение")
        })?;
    Ok(())
}
pub async fn settings(State(app): State<Arc<App>>, _m: Manager) -> ApiResult<Json<Value>> {
    Ok(Json(
        json!({"enabled":app.db.setting_or("mail.enabled","0")=="1","host":app.db.setting_or("mail.host","smtp.gmail.com"),"port":app.db.setting_or("mail.port","465").parse::<u16>().unwrap_or(465),"tls":app.db.setting_or("mail.tls","tls"),"user":app.db.setting_or("mail.user",""),"from":app.db.setting_or("mail.from",""),"base":app.db.setting_or("mail.base",""),"passwordSet":!app.db.setting_or("mail.password","").is_empty(),"ready":ready(&app)}),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsBody {
    enabled: bool,
    host: String,
    port: u16,
    tls: String,
    user: String,
    from: String,
    base: String,
    password: Option<String>,
    #[serde(default)]
    clear_password: bool,
}
pub async fn save_settings(
    State(app): State<Arc<App>>,
    _m: Manager,
    Json(b): Json<SettingsBody>,
) -> ApiResult<Json<Value>> {
    if b.host.len() > 253
        || b.host.is_empty()
        || !b.host.bytes().all(|c| c.is_ascii_alphanumeric() || b".-".contains(&c))
        || b.port == 0
        || !matches!(b.tls.as_str(), "tls" | "starttls")
    {
        return Err(ApiError::bad("smtp", "Проверьте SMTP-сервер, порт и шифрование"));
    }
    let user = if b.user.trim().is_empty() && !b.enabled { String::new() } else { address(&b.user)? };
    let from = if b.from.trim().is_empty() { user.clone() } else { address(&b.from)? };
    let base = b.base.trim().trim_end_matches('/');
    if !base.is_empty() {
        let url = reqwest::Url::parse(base).map_err(|_| ApiError::bad("url", "Проверьте публичный адрес Nova"))?;
        if !crate::outbound::valid_url(&url) || url.query().is_some() || url.fragment().is_some() || url.path() != "/" {
            return Err(ApiError::bad("url", "Укажите полный адрес Nova без пути, например https://nova.example"));
        }
    }
    let password =
        b.password.filter(|p| !p.is_empty()).map(|p| if b.host == "smtp.gmail.com" { p.replace(' ', "") } else { p });
    if password.as_ref().is_some_and(|p| p.len() > 1024) {
        return Err(ApiError::bad("smtp", "Пароль слишком длинный"));
    }
    if b.enabled && (b.clear_password || password.is_none() && app.db.setting_or("mail.password", "").is_empty()) {
        return Err(ApiError::bad("smtp", "Введите пароль приложения Gmail"));
    }
    app.db.tx(|c| {
        for (k, v) in [
            ("mail.enabled", if b.enabled { "1" } else { "0" }.into()),
            ("mail.host", b.host),
            ("mail.port", b.port.to_string()),
            ("mail.tls", b.tls),
            ("mail.user", user),
            ("mail.from", from),
            ("mail.base", base.into()),
        ] {
            c.execute(
                "INSERT INTO settings VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![k, v],
            )?;
        }
        if b.clear_password {
            c.execute("DELETE FROM settings WHERE key='mail.password'", [])?;
        } else if let Some(p) = password {
            c.execute(
                "INSERT INTO settings VALUES('mail.password',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [p],
            )?;
        }
        Ok(())
    })?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub struct EmailBody {
    email: String,
}
pub async fn test_mail(State(app): State<Arc<App>>, _m: Manager, Json(b): Json<EmailBody>) -> ApiResult<Json<Value>> {
    send(
        &app,
        &address(&b.email)?,
        "Nova · проверка отправки",
        "Письмо от Nova получено. Отправка почты работает.".into(),
    )
    .await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn email(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let email: Option<String> = app
        .db
        .with(|c| c.query_row("SELECT email FROM account_emails WHERE user_id=?", [v.id], |r| r.get(0)).optional())?;
    let pending:Option<String>=app.db.with(|c|c.query_row("SELECT email FROM account_tokens WHERE user_id=? AND purpose='verify' AND expires_at>? ORDER BY created_at DESC LIMIT 1",params![v.id,now_ms()],|r|r.get(0)).optional())?;
    Ok(Json(json!({"email":email,"pending":pending,"enabled":ready(&app)})))
}
fn throttle(app: &App, key: &str) -> ApiResult<()> {
    if app.login_limit.blocked(key) {
        return Err(ApiError::bad("rate_limited", "Слишком много попыток. Подождите 10 минут"));
    }
    app.login_limit.fail(key);
    Ok(())
}
async fn issue(app: &App, user: i64, email: &str, purpose: &str) -> ApiResult<()> {
    let raw = token(32);
    let hash = sha256_hex(raw.as_bytes());
    let path = if purpose == "verify" { "verify-email" } else { "reset-password" };
    let action = if purpose == "verify" {
        "Подтверждение почты"
    } else {
        "Восстановление пароля"
    };
    let body=format!("{action} Nova\n\nОткройте ссылку:\n{}/{path}#token={raw}\n\nСсылка действует 15 минут и используется один раз. Если вы не запрашивали это письмо, ничего делать не нужно.",base(app));
    send(app, email, &format!("Nova · {action}"), body).await?;
    app.db.tx(|c| {
        c.execute(
            "DELETE FROM account_tokens WHERE expires_at<? OR (user_id=? AND purpose=?)",
            params![now_ms(), user, purpose],
        )?;
        c.execute(
            "INSERT INTO account_tokens VALUES(?,?,?,?,?,?)",
            params![hash, user, purpose, email, now_ms() + 900_000, now_ms()],
        )?;
        Ok(())
    })?;
    Ok(())
}
#[derive(Deserialize)]
pub struct VerifyRequest {
    email: String,
    password: Option<String>,
}
pub async fn request_verify(
    State(app): State<Arc<App>>,
    v: Viewer,
    Json(b): Json<VerifyRequest>,
) -> ApiResult<Json<Value>> {
    throttle(&app, &format!("verify:{}", v.id))?;
    let email = address(&b.email)?;
    let hash: Option<String> =
        app.db.with(|c| c.query_row("SELECT pass FROM users WHERE id=?", [v.id], |r| r.get(0)))?;
    if let Some(hash) = hash {
        let password = b.password.unwrap_or_default();
        if !auth::password_job(move || auth::verify_password(&password, &hash)).await.unwrap_or(false) {
            return Err(ApiError::bad("password", "Введите текущий пароль Nova"));
        }
    }
    issue(&app, v.id, &email, "verify").await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn forgot(State(app): State<Arc<App>>, Ip(ip): Ip, Json(b): Json<EmailBody>) -> ApiResult<Json<Value>> {
    throttle(&app, &format!("forgot:{ip}"))?;
    let email = address(&b.email)?;
    let user: Option<i64> = app.db.with(|c| {
        c.query_row(
            "SELECT e.user_id FROM account_emails e JOIN users u ON u.id=e.user_id WHERE e.email=? AND u.disabled=0",
            [&email],
            |r| r.get(0),
        )
        .optional()
    })?;
    let key = format!("forgot.email.{}", sha256_hex(email.as_bytes()));
    let last = app.db.setting_or(&key, "0").parse::<i64>().unwrap_or(0);
    if ready(&app) && now_ms() - last > 60_000 {
        if let Some(user) = user {
            let _ = app.db.set_setting(&key, &now_ms().to_string());
            tokio::spawn(async move {
                if issue(&app, user, &email, "reset").await.is_err() {
                    tracing::warn!("Не удалось отправить письмо восстановления");
                }
            });
        }
    }
    Ok(Json(
        json!({"ok":true,"message":"Если эта почта подтверждена в Nova, на неё отправлена ссылка. Проверьте входящие и спам."}),
    ))
}
#[derive(Deserialize)]
pub struct TokenBody {
    token: String,
}
fn token_hash(raw: &str) -> ApiResult<String> {
    if raw.len() < 32 || raw.len() > 128 {
        return Err(ApiError::bad("token", "Ссылка недействительна или устарела"));
    }
    Ok(sha256_hex(raw.as_bytes()))
}
pub async fn verify(State(app): State<Arc<App>>, Json(b): Json<TokenBody>) -> ApiResult<Json<Value>> {
    let hash = token_hash(&b.token)?;
    let changed=app.db.tx(|c|{
        let row:Option<(i64,String)>=c.query_row("SELECT t.user_id,t.email FROM account_tokens t JOIN users u ON u.id=t.user_id WHERE t.hash=? AND t.purpose='verify' AND t.expires_at>? AND u.disabled=0",params![hash,now_ms()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let Some((user,email))=row else{return Ok(false)};
        let other:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM account_emails WHERE email=? AND user_id!=?)",params![email,user],|r|r.get(0))?;if other{return Ok(false)}
        c.execute("INSERT INTO account_emails VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET email=excluded.email",params![user,email])?;c.execute("DELETE FROM account_tokens WHERE user_id=?",[user])?;Ok(true)
    })?;
    if !changed {
        return Err(ApiError::bad("token", "Ссылка недействительна, устарела или адрес уже используется"));
    }
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetBody {
    token: String,
    new_password: String,
}
pub async fn reset(State(app): State<Arc<App>>, Ip(ip): Ip, Json(b): Json<ResetBody>) -> ApiResult<Json<Value>> {
    throttle(&app, &format!("reset:{ip}"))?;
    auth::check_password(&b.new_password)?;
    let hash = token_hash(&b.token)?;
    let user:Option<i64>=app.db.with(|c|c.query_row("SELECT t.user_id FROM account_tokens t JOIN account_emails e ON e.user_id=t.user_id AND e.email=t.email JOIN users u ON u.id=t.user_id WHERE t.hash=? AND t.purpose='reset' AND t.expires_at>? AND u.disabled=0",params![hash,now_ms()],|r|r.get(0)).optional())?;
    let user = user.ok_or_else(|| ApiError::bad("token", "Ссылка недействительна или устарела"))?;
    let password =
        auth::password_job(move || auth::hash_password(&b.new_password)).await.map_err(ApiError::internal)?;
    let changed = app.db.tx(|c| {
        let used = c.execute(
            "DELETE FROM account_tokens WHERE hash=? AND purpose='reset' AND expires_at>?",
            params![hash, now_ms()],
        )?;
        if used != 1 {
            return Ok(false);
        }
        c.execute("UPDATE users SET pass=? WHERE id=? AND disabled=0", params![password, user])?;
        c.execute("DELETE FROM account_tokens WHERE user_id=?", [user])?;
        c.execute("DELETE FROM sessions WHERE user_id=? AND local=0", [user])?;
        Ok(true)
    })?;
    if !changed {
        return Err(ApiError::bad("token", "Ссылка уже использована или устарела"));
    }
    Ok(Json(json!({"ok":true})))
}
