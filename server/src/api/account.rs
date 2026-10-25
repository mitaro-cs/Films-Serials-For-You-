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

fn site_json(app: &App) -> Value {
    json!({
        "name": app.site_name(),
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
    let row: Option<(i64, Option<String>, bool)> = app.db.with(|c| {
        c.query_row("SELECT id, pass, disabled FROM users WHERE login = ?", [&login], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()
    })?;
    let pw = b.password;
    let ok = match row.clone() {
        Some((_, Some(hash), false)) => {
            tokio::task::spawn_blocking(move || auth::verify_password(&pw, &hash)).await.unwrap_or(false)
        }
        _ => {
            // То же время ответа, что и с паролем: не выдаём, есть ли такой логин.
            let _ = tokio::task::spawn_blocking(move || auth::hash_password(&pw)).await;
            false
        }
    };
    if !ok {
        keys.iter().for_each(|k| app.login_limit.fail(k));
        app.account_limit.fail(&account);
        if matches!(row, Some((_, _, true))) {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "disabled", "Доступ закрыт хостом"));
        }
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "bad_credentials", "Неверный логин или пароль"));
    }
    let id = row.map(|r| r.0).unwrap_or_default();
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

/// Регистрация по умолчанию — открытая: друзья заводят аккаунт сами, без приглашений
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
    // Новых аккаунтов с одного адреса — не больше 5 за 10 минут (от ботов при открытой регистрации).
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
            "Регистрация закрыта — попросите приглашение у хоста",
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
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&pw)).await.map_err(ApiError::internal)?;
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
        "manage": v.manages(),
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

/// Имя (у аккаунта и профиля — одно), рисованный аватар, автозапуск следующей серии.
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
        let ok = tokio::task::spawn_blocking(move || auth::verify_password(&old, &hash)).await.unwrap_or(false);
        if !ok {
            app.login_limit.fail(&key);
            return Err(ApiError::bad("bad_password", "Текущий пароль не подошёл"));
        }
    }
    let new = b.new;
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&new)).await.map_err(ApiError::internal)?;
    // Новый пароль — остальные входы закрываются (кроме этого и окна хоста): если пароль меняют
    // из-за утечки, украденная сессия больше не работает.
    app.db.tx(|tx| {
        tx.execute("UPDATE users SET pass = ? WHERE id = ?", params![hash, v.id])?;
        tx.execute("DELETE FROM sessions WHERE user_id = ? AND hash != ? AND local = 0", params![v.id, v.token_hash])
    })?;
    Ok(Json(json!({"ok": true, "login": v.login})))
}
