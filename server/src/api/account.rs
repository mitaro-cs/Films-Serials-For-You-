//! Вход, регистрация по приглашению, свой профиль, вход окна хоста.

use crate::auth::{self, client_ip, Via, Viewer};
use crate::error::{ApiError, ApiResult};
use crate::util::now_ms;
use crate::App;
use axum::extract::{Path, Query, State};
use axum::http::{header, request::Parts, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Json};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn health() -> Json<Value> {
    Json(json!({"ok": true, "version": crate::VERSION}))
}

#[derive(Deserialize)]
pub struct NightBody {
    action: String,
    #[serde(default)]
    lights: bool,
}
pub async fn night(State(app): State<Arc<App>>, v: Viewer, Json(body): Json<NightBody>) -> ApiResult<Json<Value>> {
    if !v.local {
        return Err(ApiError::not_found());
    }
    if !matches!(body.action.as_str(), "off" | "restore" | "probe") {
        return Err(ApiError::bad("action", "Неизвестная команда ночного режима"));
    }
    let host = app.host.clone();
    let result =
        tokio::task::spawn_blocking(move || host.night(&body.action, body.lights)).await.map_err(ApiError::internal)?;
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct FullscreenBody {
    enabled: bool,
}
pub async fn fullscreen(
    State(app): State<Arc<App>>,
    v: Viewer,
    Json(body): Json<FullscreenBody>,
) -> ApiResult<Json<Value>> {
    if !v.local {
        return Err(ApiError::not_found());
    }
    Ok(Json(json!({"native":app.host.fullscreen(body.enabled)})))
}

fn site_json(app: &App) -> Value {
    json!({
        "name": app.site_name(),
        "metadataLanguage": app.db.setting_or("meta.language", "ru"),
        "emailRecovery": super::mail::ready(app),
        "registration": app.db.setting_or("site.registration", crate::api::account::REGISTRATION_DEFAULT),
        "downloads": app.db.setting_or("site.downloads", "1") == "1",
        "version": crate::VERSION,
    })
}

pub async fn site(State(app): State<Arc<App>>) -> Json<Value> {
    Json(site_json(&app))
}

/// Части запроса для адреса клиента (ConnectInfo/X-Forwarded-For).
pub struct Ip(pub String);

impl axum::extract::FromRequestParts<Arc<App>> for Ip {
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(parts: &mut Parts, _: &Arc<App>) -> Result<Self, Self::Rejection> {
        Ok(Ip(client_ip(parts)))
    }
}

fn with_cookie(body: Value, cookie: String) -> Response {
    ([(header::SET_COOKIE, cookie)], Json(body)).into_response()
}

#[derive(Deserialize)]
pub struct LoginBody {
    login: String,
    password: String,
    /// Код из приложения - если у аккаунта включён вход в два шага.
    #[serde(default)]
    code: Option<String>,
}

/// Журнал входов (без адресов: о людях храним минимум) - последние 500 записей.
fn journal(app: &App, user: Option<i64>, login: &str, ok: bool, headers: &HeaderMap) {
    let agent: Option<String> =
        headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(|a| a.chars().take(160).collect());
    let _ = app.db.with(|c| {
        c.execute(
            "INSERT INTO logins (user_id, login, ok, via, agent, at) VALUES (?, ?, ?, 'password', ?, ?)",
            params![user, login.chars().take(64).collect::<String>(), ok, agent, now_ms()],
        )?;
        c.execute("DELETE FROM logins WHERE id <= (SELECT MAX(id) FROM logins) - 500", [])
    });
}

pub async fn login(
    State(app): State<Arc<App>>,
    Ip(ip): Ip,
    headers: HeaderMap,
    Json(b): Json<LoginBody>,
) -> ApiResult<Response> {
    let login = b.login.trim().to_lowercase();
    // С адреса — 10 неудач за 10 минут; в один аккаунт со всех адресов — 50: чужие 10 попыток
    // больше не запирают вход хозяину аккаунта.
    let keys = [format!("ip:{ip}"), format!("pair:{ip}:{login}")];
    let account = format!("login:{login}");
    if keys.iter().any(|k| app.login_limit.blocked(k)) || app.account_limit.blocked(&account) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Слишком много попыток. Подождите 10 минут",
        ));
    }
    let row: Option<(i64, Option<String>, bool, Option<String>)> = app.db.with(|c| {
        c.query_row("SELECT id, pass, disabled, totp FROM users WHERE login = ?", [&login], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()
    })?;
    let pw = b.password;
    let ok = match row.clone() {
        Some((_, Some(hash), false, _)) => {
            auth::password_job(move || auth::verify_password(&pw, &hash)).await.unwrap_or(false)
        }
        _ => {
            // То же время ответа, что и с паролем: не выдаём, есть ли такой логин.
            let _ = auth::password_job(move || auth::hash_password(&pw)).await;
            false
        }
    };
    // Второй шаг: пароль верный - нужен ещё код из приложения.
    let totp = row.as_ref().and_then(|r| r.3.clone()).filter(|_| ok);
    if let Some(secret) = &totp {
        match b.code.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            None => {
                return Err(ApiError::new(StatusCode::UNAUTHORIZED, "totp_required", "Введите код из приложения"));
            }
            Some(code) if !crate::totp::check(secret, code, now_ms()) => {
                keys.iter().for_each(|k| app.login_limit.fail(k));
                app.account_limit.fail(&account);
                journal(&app, row.as_ref().map(|r| r.0), &login, false, &headers);
                tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                return Err(ApiError::new(StatusCode::UNAUTHORIZED, "bad_totp", "Неверный код из приложения"));
            }
            Some(_) => {}
        }
    }
    if !ok {
        keys.iter().for_each(|k| app.login_limit.fail(k));
        app.account_limit.fail(&account);
        journal(&app, row.as_ref().map(|r| r.0), &login, false, &headers);
        // Каждая неудача - ещё и пауза: подбор пароля идёт в разы медленнее.
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        if matches!(row, Some((_, _, true, _))) {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "disabled", "Доступ закрыт администрацией Nova"));
        }
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "bad_credentials", "Неверный логин или пароль"));
    }
    let id = row.map(|r| r.0).unwrap_or_default();
    journal(&app, Some(id), &login, true, &headers);
    keys.iter().for_each(|k| app.login_limit.clear(k));
    app.account_limit.clear(&account);
    let t = auth::create_session(&app, id, false)?;
    Ok(with_cookie(json!({"ok": true}), auth::session_cookie(auth::COOKIE, &t, auth::secure(&headers), 365 * 86400)))
}

pub async fn logout(State(app): State<Arc<App>>, Extension(via): Extension<Via>, headers: HeaderMap) -> Response {
    let name = if via.window { auth::WINDOW_COOKIE } else { auth::COOKIE };
    if let Some(t) = auth::cookie_value(&headers, name) {
        auth::drop_session(&app, &t);
    }
    with_cookie(json!({"ok": true}), auth::session_cookie(name, "", auth::secure(&headers), 0))
}

fn invite_ok(app: &App, code: &str) -> ApiResult<bool> {
    let row: Option<(Option<i64>, Option<i64>)> = app.db.with(|c| {
        c.query_row("SELECT uses_left, expires_at FROM invites WHERE code = ?", [code.trim().to_uppercase()], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
    })?;
    Ok(matches!(row, Some((uses, exp)) if uses.is_none_or(|u| u > 0) && exp.is_none_or(|e| e > now_ms())))
}

pub async fn invite(State(app): State<Arc<App>>, Ip(ip): Ip, Path(code): Path<String>) -> ApiResult<Json<Value>> {
    let key = format!("invite:{ip}");
    if app.login_limit.blocked(&key) || app.site_limit.blocked("invite") {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Слишком много попыток. Подождите 10 минут",
        ));
    }
    let ok = invite_ok(&app, &code)?;
    if !ok {
        app.login_limit.fail(&key);
        app.site_limit.fail("invite");
    }
    Ok(Json(json!({"valid": ok, "site": app.site_name()})))
}

#[derive(Deserialize)]
pub struct RegisterBody {
    invite: Option<String>,
    name: String,
    login: String,
    password: String,
}

/// Регистрация по умолчанию - открытая: друзья заводят аккаунт сами, без приглашений
/// (просьба владельца, 0.4). Хост может включить «по приглашениям» или закрыть.
pub const REGISTRATION_DEFAULT: &str = "open";

pub async fn register(
    State(app): State<Arc<App>>,
    Ip(ip): Ip,
    headers: HeaderMap,
    Json(b): Json<RegisterBody>,
) -> ApiResult<Response> {
    let mode = app.db.setting_or("site.registration", crate::api::account::REGISTRATION_DEFAULT);
    let key = format!("invite:{ip}");
    if app.login_limit.blocked(&key) || app.site_limit.blocked("invite") {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Слишком много попыток. Подождите 10 минут",
        ));
    }
    // Новых аккаунтов с одного адреса - не больше 5 за 10 минут (от ботов при открытой регистрации).
    let signups = format!("signup:{ip}");
    if app.signup_limit.blocked(&signups) || app.site_limit.blocked("signup") {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "С этого адреса уже заводили аккаунты. Подождите 10 минут",
        ));
    }
    let code = b.invite.as_deref().map(|c| c.trim().to_uppercase()).filter(|c| !c.is_empty());
    let need_invite = mode != "open";
    if mode == "closed" && code.is_none() {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "closed",
            "Регистрация закрыта - попросите приглашение у администрации Nova",
        ));
    }
    if need_invite && !code.as_deref().map(|c| invite_ok(&app, c)).transpose()?.unwrap_or(false) {
        app.login_limit.fail(&key);
        app.site_limit.fail("invite");
        return Err(ApiError::bad("bad_invite", "Приглашение недействительно или закончилось"));
    }
    let name = auth::check_name(&b.name)?;
    let login = auth::check_login(&b.login)?;
    auth::check_password(&b.password)?;
    let pw = b.password.clone();
    let hash = auth::password_job(move || auth::hash_password(&pw)).await.map_err(ApiError::internal)?;
    let id = app.db.tx(|tx| {
        let taken: bool = tx.query_row("SELECT EXISTS (SELECT 1 FROM users WHERE login = ?)", [&login], |r| r.get(0))?;
        if taken {
            return Ok(None);
        }
        if let Some(c) = code.as_deref().filter(|_| need_invite) {
            tx.execute("UPDATE invites SET used = used + 1, uses_left = CASE WHEN uses_left IS NULL THEN NULL ELSE uses_left - 1 END WHERE code = ?", [c])?;
        }
        tx.execute(
            "INSERT INTO users (login, name, pass, role, created_at) VALUES (?, ?, ?, 'viewer', ?)",
            params![login, name, hash, now_ms()],
        )?;
        Ok(Some(tx.last_insert_rowid()))
    })?;
    let Some(id) = id else {
        return Err(ApiError::new(StatusCode::CONFLICT, "login_taken", "Такой логин уже занят"));
    };
    app.signup_limit.fail(&signups);
    app.site_limit.fail("signup");
    let t = auth::create_session(&app, id, false)?;
    Ok(with_cookie(json!({"ok": true}), auth::session_cookie(auth::COOKIE, &t, auth::secure(&headers), 365 * 86400)))
}

#[derive(Deserialize)]
pub struct EnterQuery {
    t: String,
}

/// Вход окна хоста по одноразовой ссылке. Отвечает страницей, а не 302: окно ставит cookie
/// и само переходит на главную.
pub async fn enter(
    State(app): State<Arc<App>>,
    Extension(via): Extension<Via>,
    Query(q): Query<EnterQuery>,
) -> ApiResult<Response> {
    if !via.window {
        return Err(ApiError::forbidden());
    }
    let ok = app.entries.lock().remove(&q.t).is_some_and(|exp| exp > now_ms());
    if !ok {
        return Ok(Html("<!doctype html><meta charset=utf-8><script>location.replace('/')</script>").into_response());
    }
    let host: i64 = app
        .db
        .with(|c| c.query_row("SELECT id FROM users WHERE role = 'host' ORDER BY id LIMIT 1", [], |r| r.get(0)))?;
    let t = auth::create_session(&app, host, true)?;
    Ok((
        [(header::SET_COOKIE, auth::session_cookie(auth::WINDOW_COOKIE, &t, false, 365 * 86400))],
        Html("<!doctype html><meta charset=utf-8><title>Вход</title><style>html{background:#0b0a14}</style><script>location.replace('/')</script>"),
    )
        .into_response())
}

pub async fn me(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let has_pass: bool =
        app.db.with(|c| c.query_row("SELECT pass IS NOT NULL FROM users WHERE id = ?", [v.id], |r| r.get(0)))?;
    let favorites: String =
        app.db.with(|c| c.query_row("SELECT favorites FROM profiles WHERE id = ?", [v.profile.id], |r| r.get(0)))?;
    Ok(Json(json!({
        "user": v,
        "favorites": super::json_list(Some(favorites)),
        "hasPassword": has_pass,
        "manage": v.manages() && auth::manage_here(&app, &v),
        "window": v.local,
        "site": site_json(&app),
    })))
}

#[derive(Deserialize)]
pub struct MeBody {
    name: Option<String>,
    avatar: Option<i64>,
    autoplay: Option<bool>,
}

/// Имя (у аккаунта и профиля - одно), рисованный аватар, автозапуск следующей серии.
pub async fn update_me(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<MeBody>) -> ApiResult<Json<Value>> {
    let name = b.name.as_deref().map(auth::check_name).transpose()?;
    app.db.tx(|tx| {
        if let Some(n) = &name {
            tx.execute("UPDATE users SET name = ? WHERE id = ?", params![n, v.id])?;
            tx.execute("UPDATE profiles SET name = ? WHERE id = ?", params![n, v.profile.id])?;
        }
        if let Some(a) = b.avatar {
            let a = a.rem_euclid(super::profiles::AVATARS);
            tx.execute("UPDATE profiles SET avatar = ? WHERE id = ?", params![a, v.profile.id])?;
        }
        if let Some(a) = b.autoplay {
            tx.execute("UPDATE profiles SET autoplay = ? WHERE id = ?", params![a, v.profile.id])?;
        }
        Ok(())
    })?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct PasswordBody {
    old: Option<String>,
    new: String,
}

pub async fn password(
    State(app): State<Arc<App>>,
    v: Viewer,
    Ip(ip): Ip,
    Json(b): Json<PasswordBody>,
) -> ApiResult<Json<Value>> {
    auth::check_password(&b.new)?;
    let key = format!("ip:{ip}");
    if app.login_limit.blocked(&key) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Слишком много попыток. Подождите 10 минут",
        ));
    }
    let current: Option<String> =
        app.db.with(|c| c.query_row("SELECT pass FROM users WHERE id = ?", [v.id], |r| r.get(0)))?;
    if let Some(hash) = current {
        let old = b.old.unwrap_or_default();
        let ok = auth::password_job(move || auth::verify_password(&old, &hash)).await.unwrap_or(false);
        if !ok {
            app.login_limit.fail(&key);
            return Err(ApiError::bad("bad_password", "Текущий пароль не подошёл"));
        }
    }
    let new = b.new;
    let hash = auth::password_job(move || auth::hash_password(&new)).await.map_err(ApiError::internal)?;
    // Новый пароль - остальные входы закрываются (кроме этого и окна хоста): если пароль меняют
    // из-за утечки, украденная сессия больше не работает. Ссылки восстановления - тоже.
    app.db.tx(|tx| {
        tx.execute("UPDATE users SET pass = ? WHERE id = ?", params![hash, v.id])?;
        tx.execute("DELETE FROM account_tokens WHERE user_id = ?", [v.id])?;
        tx.execute("DELETE FROM sessions WHERE user_id = ? AND hash != ? AND local = 0", params![v.id, v.token_hash])?;
        Ok(())
    })?;
    Ok(Json(json!({"ok": true, "login": v.login})))
}

// ---------- вход в два шага и журнал входов ----------

/// Безопасность аккаунта: включён ли второй шаг и последние входы в этот аккаунт.
pub async fn security(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let totp: Option<String> =
        app.db.with(|c| c.query_row("SELECT totp FROM users WHERE id = ?", [v.id], |r| r.get(0)))?;
    let logins: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare("SELECT ok, agent, at FROM logins WHERE user_id = ? ORDER BY id DESC LIMIT 20")?;
        let r = st.query_map([v.id], |r| {
            Ok(json!({"ok": r.get::<_, bool>(0)?, "agent": r.get::<_, Option<String>>(1)?, "at": r.get::<_, i64>(2)?}))
        })?;
        r.collect()
    })?;
    Ok(Json(json!({"totp": totp.is_some(), "logins": logins})))
}

/// Начать включение: новый ключ (пока не сохранён - только после верного кода).
pub async fn totp_setup(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let secret = crate::totp::new_secret();
    app.db.set_setting(&format!("totp.pending.{}", v.id), &format!("{secret}|{}", now_ms()))?;
    Ok(Json(json!({"secret": secret, "uri": crate::totp::uri(&secret, &v.login)})))
}

#[derive(Deserialize)]
pub struct TotpBody {
    code: String,
}

/// Включить: код из приложения по ключу из `totp_setup` (ключ живёт 15 минут).
pub async fn totp_enable(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<TotpBody>) -> ApiResult<Json<Value>> {
    let key = format!("totp.pending.{}", v.id);
    let pending = app.db.setting(&key).unwrap_or_default();
    let (secret, at) = pending.split_once('|').unwrap_or(("", "0"));
    if secret.is_empty() || now_ms() - at.parse::<i64>().unwrap_or(0) > 15 * 60_000 {
        return Err(ApiError::bad("totp_expired", "Ключ устарел - начните заново"));
    }
    if app.login_limit.blocked(&format!("totp:{}", v.id)) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Слишком много попыток. Подождите 10 минут",
        ));
    }
    if !crate::totp::check(secret, &b.code, now_ms()) {
        app.login_limit.fail(&format!("totp:{}", v.id));
        return Err(ApiError::bad("bad_totp", "Код не подошёл - проверьте время на телефоне"));
    }
    app.db.with(|c| {
        c.execute("UPDATE users SET totp = ? WHERE id = ?", params![secret, v.id])?;
        c.execute("DELETE FROM settings WHERE key = ?", [&key])
    })?;
    Ok(Json(json!({"ok": true})))
}

/// Выключить: нужен текущий код (украденная сессия сама второй шаг не снимет).
pub async fn totp_disable(State(app): State<Arc<App>>, v: Viewer, Json(b): Json<TotpBody>) -> ApiResult<Json<Value>> {
    let secret: Option<String> =
        app.db.with(|c| c.query_row("SELECT totp FROM users WHERE id = ?", [v.id], |r| r.get(0)))?;
    let Some(secret) = secret else { return Ok(Json(json!({"ok": true}))) };
    if app.login_limit.blocked(&format!("totp:{}", v.id)) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Слишком много попыток. Подождите 10 минут",
        ));
    }
    if !crate::totp::check(&secret, &b.code, now_ms()) {
        app.login_limit.fail(&format!("totp:{}", v.id));
        return Err(ApiError::bad("bad_totp", "Неверный код из приложения"));
    }
    app.db.with(|c| c.execute("UPDATE users SET totp = NULL WHERE id = ?", [v.id]))?;
    Ok(Json(json!({"ok": true})))
}
