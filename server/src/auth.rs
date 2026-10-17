//! Вход: пароли Argon2id, сессии в cookie (в базе — только хеш токена), роли, защита от подбора.
//!
//! Окно приложения хоста ходит на отдельный порт (только 127.0.0.1) и получает «локальную»
//! сессию хоста без пароля. Такая сессия действует **только** через этот порт: туннель тоже
//! подключается с 127.0.0.1, но к другому порту.

use crate::error::{ApiError, ApiResult};
use crate::util::{now_ms, sha256_hex, token};
use crate::App;
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{ConnectInfo, FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap};
use parking_lot::Mutex;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

pub const COOKIE: &str = "fsfy_s";
pub const WINDOW_COOKIE: &str = "fsfy_w";
const YEAR_MS: i64 = 365 * 24 * 3600 * 1000;

/// С какого входа пришёл запрос: окно хоста (127.0.0.1, свой порт) или общий.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Via {
    pub window: bool,
}

pub fn hash_password(pw: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(pw.as_bytes(), &salt).map(|h| h.to_string()).unwrap_or_default()
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash).map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok()).unwrap_or(false)
}

pub fn check_password(pw: &str) -> ApiResult<()> {
    if pw.chars().count() < 6 {
        return Err(ApiError::bad("weak_password", "Пароль — не короче 6 символов"));
    }
    if pw.len() > 200 {
        return Err(ApiError::bad("weak_password", "Слишком длинный пароль"));
    }
    Ok(())
}

pub fn check_login(login: &str) -> ApiResult<String> {
    let l = login.trim().to_lowercase();
    let ok = (3..=32).contains(&l.chars().count())
        && l.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.');
    if !ok {
        return Err(ApiError::bad("bad_login", "Логин: 3–32 символа, буквы, цифры, «_», «-», «.»"));
    }
    Ok(l)
}

pub fn check_name(name: &str) -> ApiResult<String> {
    let n = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() || n.chars().count() > 60 {
        return Err(ApiError::bad("bad_name", "Имя — от 1 до 60 символов"));
    }
    Ok(n)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewer {
    pub id: i64,
    pub login: String,
    pub name: String,
    pub role: String,
    pub can_download: bool,
    #[serde(skip)]
    pub local: bool,
    /// Профиль («Кто смотрит?»): прогресс, список и оценки — у него.
    pub profile: Profile,
    /// Профиль выбран в этой сессии (иначе интерфейс спросит «Кто смотрит?»).
    pub profile_chosen: bool,
    #[serde(skip)]
    pub token_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub avatar: i64,
    pub kids: bool,
    pub autoplay: bool,
}

impl Profile {
    pub fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Profile { id: r.get(0)?, name: r.get(1)?, avatar: r.get(2)?, kids: r.get(3)?, autoplay: r.get(4)? })
    }
}

pub const PROFILE_COLS: &str = "id, name, avatar, kids, autoplay";

/// Профиль сессии; нет ни одного (новый аккаунт) — создаётся с именем аккаунта.
fn profile_for(app: &App, user: i64, name: &str, wanted: Option<i64>) -> rusqlite::Result<Profile> {
    app.db.with(|c| {
        let found = c
            .query_row(
                &format!("SELECT {PROFILE_COLS} FROM profiles WHERE user_id = ? ORDER BY id = ? DESC, id LIMIT 1"),
                params![user, wanted.unwrap_or(-1)],
                Profile::from_row,
            )
            .optional()?;
        match found {
            Some(p) => Ok(p),
            None => {
                c.execute(
                    "INSERT INTO profiles (user_id, name, avatar, created_at) VALUES (?, ?, ?, ?)",
                    params![user, name, user % 12, now_ms()],
                )?;
                Ok(Profile {
                    id: c.last_insert_rowid(),
                    name: name.to_string(),
                    avatar: user % 12,
                    kids: false,
                    autoplay: true,
                })
            }
        }
    })
}

impl Viewer {
    pub fn manages(&self) -> bool {
        self.role == "host" || self.role == "admin"
    }
}

/// Тот, кто управляет сайтом: хост или администратор.
pub struct Manager(pub Viewer);

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(';')).find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == name).then(|| v.to_string())
    })
}

pub fn client_ip(parts: &Parts) -> String {
    let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip());
    if peer.is_none_or(|p| p.is_loopback()) {
        // Через туннель все приходят с 127.0.0.1 — настоящий адрес в X-Forwarded-For.
        if let Some(f) = parts.headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(ip) = f.split(',').next().and_then(|s| s.trim().parse::<IpAddr>().ok()) {
                return ip.to_string();
            }
        }
    }
    peer.map(|p| p.to_string()).unwrap_or_else(|| "?".into())
}

pub fn via(parts: &Parts) -> Via {
    parts.extensions.get::<Via>().copied().unwrap_or(Via { window: false })
}

pub fn secure(headers: &HeaderMap) -> bool {
    headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()).is_some_and(|p| p.eq_ignore_ascii_case("https"))
}

pub fn session_cookie(name: &str, token: &str, secure: bool, max_age_s: i64) -> String {
    format!(
        "{name}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_s}{}",
        if secure { "; Secure" } else { "" }
    )
}

/// Новая сессия; возвращает токен для cookie.
pub fn create_session(app: &App, user_id: i64, local: bool) -> ApiResult<String> {
    let t = token(32);
    let now = now_ms();
    app.db.with(|c| {
        c.execute(
            "INSERT INTO sessions (hash, user_id, local, created_at, seen_at, expires_at) VALUES (?, ?, ?, ?, ?, ?)",
            params![sha256_hex(t.as_bytes()), user_id, local, now, now, now + YEAR_MS],
        )
    })?;
    Ok(t)
}

pub fn drop_session(app: &App, token: &str) {
    let _ = app.db.with(|c| c.execute("DELETE FROM sessions WHERE hash = ?", [sha256_hex(token.as_bytes())]));
}

fn load(app: &App, parts: &Parts) -> Option<Viewer> {
    let via = via(parts);
    let t = cookie_value(&parts.headers, if via.window { WINDOW_COOKIE } else { COOKIE })?;
    let hash = sha256_hex(t.as_bytes());
    let now = now_ms();
    let row = app
        .db
        .with(|c| {
            c.query_row(
                "SELECT u.id, u.login, u.name, u.role, u.can_download, s.local, s.seen_at, s.expires_at, s.profile_id
                 FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.hash = ? AND u.disabled = 0",
                [&hash],
                |r| {
                    Ok((
                        (
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, bool>(4)?,
                            r.get::<_, bool>(5)?,
                        ),
                        r.get::<_, i64>(6)?,
                        r.get::<_, i64>(7)?,
                        r.get::<_, Option<i64>>(8)?,
                    ))
                },
            )
            .optional()
        })
        .ok()??;
    let ((id, login, name, role, can_download, local), seen, expires, wanted) = row;
    if expires < now || local && !via.window {
        return None;
    }
    let profile = profile_for(app, id, &name, wanted).ok()?;
    let profile_chosen = wanted == Some(profile.id);
    let v = Viewer { id, login, name, role, can_download, local, profile, profile_chosen, token_hash: hash.clone() };
    // Скользящий срок: продлеваем не чаще раза в час, чтобы не писать на каждый запрос.
    if now - seen > 3600 * 1000 {
        let _ = app.db.with(|c| {
            c.execute(
                "UPDATE sessions SET seen_at = ?, expires_at = ? WHERE hash = ?",
                params![now, now + YEAR_MS, hash],
            )?;
            c.execute("UPDATE users SET seen_at = ? WHERE id = ?", params![now, v.id])
        });
    }
    Some(v)
}

impl FromRequestParts<Arc<App>> for Viewer {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Self, Self::Rejection> {
        load(app, parts).ok_or_else(ApiError::unauthorized)
    }
}

impl OptionalFromRequestParts<Arc<App>> for Viewer {
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Option<Self>, Self::Rejection> {
        Ok(load(app, parts))
    }
}

impl FromRequestParts<Arc<App>> for Manager {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Self, Self::Rejection> {
        let v = load(app, parts).ok_or_else(ApiError::unauthorized)?;
        if !v.manages() {
            return Err(ApiError::forbidden());
        }
        Ok(Manager(v))
    }
}

/// Не больше `max` неудач за `window_ms` на ключ (адрес, логин, код).
pub struct Limiter {
    hits: Mutex<HashMap<String, Vec<i64>>>,
    max: usize,
    window_ms: i64,
}

impl Limiter {
    pub fn new(max: usize, window_ms: i64) -> Self {
        Self { hits: Mutex::new(HashMap::new()), max, window_ms }
    }

    pub fn blocked(&self, key: &str) -> bool {
        let now = now_ms();
        let mut h = self.hits.lock();
        let v = h.entry(key.to_string()).or_default();
        v.retain(|t| now - t < self.window_ms);
        v.len() >= self.max
    }

    pub fn fail(&self, key: &str) {
        let mut h = self.hits.lock();
        if h.len() > 10_000 {
            let now = now_ms();
            let w = self.window_ms;
            h.retain(|_, v| v.iter().any(|t| now - t < w));
        }
        h.entry(key.to_string()).or_default().push(now_ms());
    }

    pub fn clear(&self, key: &str) {
        self.hits.lock().remove(key);
    }
}

/// Хост есть всегда: он входит из окна приложения, пароль можно задать для входа с телефона.
pub fn ensure_host(app: &App) -> rusqlite::Result<()> {
    app.db.with(|c| {
        let has: bool = c.query_row("SELECT EXISTS (SELECT 1 FROM users WHERE role = 'host')", [], |r| r.get(0))?;
        if !has {
            c.execute(
                "INSERT INTO users (login, name, role, created_at) VALUES ('host', 'Хост', 'host', ?)",
                [now_ms()],
            )?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords() {
        let h = hash_password("secret1");
        assert!(verify_password("secret1", &h));
        assert!(!verify_password("secret2", &h));
        assert!(!verify_password("x", "garbage"));
    }

    #[test]
    fn limiter_blocks_after_max() {
        let l = Limiter::new(2, 60_000);
        assert!(!l.blocked("a"));
        l.fail("a");
        l.fail("a");
        assert!(l.blocked("a"));
        l.clear("a");
        assert!(!l.blocked("a"));
    }

    #[test]
    fn logins() {
        assert_eq!(check_login(" Vasya_1 ").unwrap(), "vasya_1");
        assert!(check_login("a").is_err());
        assert!(check_login("a b c").is_err());
    }
}
