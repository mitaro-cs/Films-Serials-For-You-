//! Доступ к сайту для зрителей:
//! - `lan` — только домашняя сеть (`http://192.168.…:порт`);
//! - `fxtunnel` — бесплатный туннель fxTunnel (`https://имя.fxtun.ru`, работает из России,
//!   белый IP не нужен). Клиент скачивается с GitHub и сверяется по SHA-256;
//! - `custom` — свой адрес (проброс порта на роутере, свой домен).

use crate::host::HostEvent;
use crate::util::now_ms;
use crate::App;
use parking_lot::Mutex;
use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Notify;

pub const FX_API: &str = "https://fxtun.ru";
pub const FX_DOMAIN: &str = "fxtun.ru";
const FX_VERSION: &str = "v3.12.0";
const FX_SHA256: &[(&str, &str)] = &[
    ("darwin-amd64", "3d52e7c4c982ca0ccef29d8022e46f734b0dfc7298661138e5d29f7418b821e2"),
    ("darwin-arm64", "324622e119c0ebb09b8ca1bd3fd1c62e1346c57b4446a323b8ab6d7f8d6bec10"),
    ("linux-amd64", "6708d1ccc43ac2f628dc82e07ede4c0c92bc4fca7b948fdd70bf73c9d6a7dff9"),
    ("linux-arm64", "0afeda5d2e03a7a5ce32408a44a1b8a762609aa7a66a78ee82c5383dbf10cf7a"),
    ("windows-amd64", "b0ba043d6a86271244b430573f3d61d6184ab129d32951126fa77bc6ebf25b2a"),
];
const BACKOFF: &[u64] = &[5, 15, 30, 60, 120];

static HTTPS_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"HTTPS:\s*(https://\S+)").unwrap());
static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new("\u{1b}\\[[0-9;?]*[A-Za-z]").unwrap());
static FAILED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)failed to connect:\s*(.*)").unwrap());
static AUTH_ERROR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(unauthori[sz]ed|invalid token|token (is )?(invalid|expired)|401)").unwrap());
static TAKEN_ERROR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(domain|subdomain).*(taken|in use|already|unavailable|reserved)").unwrap());
static LIMIT_ERROR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(limit|too many tunnels)").unwrap());
pub static SUBDOMAIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9](?:[a-z0-9-]{1,30}[a-z0-9])$").unwrap());

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginView {
    pub code: String,
    pub url: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessView {
    pub mode: String,
    /// off, starting, online, retrying, needs_login, error
    pub state: String,
    pub message: Option<String>,
    pub public_url: Option<String>,
    pub lan_url: Option<String>,
    pub subdomain: String,
    pub fx_logged_in: bool,
    pub login: Option<LoginView>,
    pub custom_url: String,
}

#[derive(Default)]
struct Inner {
    state: String,
    message: Option<String>,
    url: Option<String>,
    gen: u64,
    login: Option<LoginView>,
}

#[derive(Default)]
pub struct Access {
    inner: Mutex<Inner>,
    stop: Notify,
}

/// Адрес этого компьютера в домашней сети (без отправки пакетов — только выбор маршрута).
pub fn lan_ip() -> Option<std::net::IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

fn platform() -> Option<String> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return None,
    };
    Some(format!("{os}-{arch}"))
}

impl Access {
    pub fn mode(app: &App) -> String {
        app.db.setting_or("access.mode", "lan")
    }

    pub fn view(&self, app: &App) -> AccessView {
        let i = self.inner.lock();
        let mode = Self::mode(app);
        let custom = app.db.setting_or("access.url", "");
        AccessView {
            public_url: match mode.as_str() {
                "custom" if !custom.is_empty() => Some(custom.clone()),
                "fxtunnel" => i.url.clone(),
                _ => None,
            },
            state: match mode.as_str() {
                "lan" => "off".into(),
                "custom" => {
                    if custom.is_empty() {
                        "error".into()
                    } else {
                        "online".into()
                    }
                }
                _ => {
                    if i.state.is_empty() {
                        "starting".into()
                    } else {
                        i.state.clone()
                    }
                }
            },
            message: if mode == "fxtunnel" { i.message.clone() } else { None },
            lan_url: lan_ip().map(|ip| format!("http://{ip}:{}", app.ports.0)),
            subdomain: app.db.setting_or("access.fx.sub", ""),
            fx_logged_in: !app.db.setting_or("access.fx.token", "").is_empty(),
            login: i.login.clone().filter(|l| l.expires_at > now_ms()),
            custom_url: custom,
            mode,
        }
    }

    /// Адрес, по которому зрители открывают сайт (для ссылок-приглашений).
    pub fn public_url(&self, app: &App) -> Option<String> {
        let v = self.view(app);
        v.public_url.or(v.lan_url)
    }

    fn set(&self, app: &App, gen: u64, state: &str, message: Option<String>, url: Option<Option<String>>) {
        {
            let mut i = self.inner.lock();
            if i.gen != gen {
                return;
            }
            i.state = state.into();
            i.message = message;
            if let Some(u) = url {
                i.url = u;
            }
        }
        let v = self.view(app);
        app.host.event(HostEvent::Access { url: v.public_url, lan: v.lan_url });
    }

    /// Применить режим из настроек: остановить старое, запустить новое.
    pub fn restart(app: &Arc<App>) {
        let gen = {
            let mut i = app.access.inner.lock();
            i.gen += 1;
            i.state.clear();
            i.message = None;
            i.url = None;
            i.gen
        };
        app.access.stop.notify_waiters();
        let a = app.clone();
        let v = app.access.view(app);
        app.host.event(HostEvent::Access { url: v.public_url, lan: v.lan_url });
        if Self::mode(app) == "fxtunnel" {
            tokio::spawn(async move { Self::run(a, gen).await });
        }
    }

    fn current(&self, gen: u64) -> bool {
        self.inner.lock().gen == gen
    }

    async fn run(app: Arc<App>, gen: u64) {
        let mut failures = 0usize;
        loop {
            let token = app.db.setting_or("access.fx.token", "");
            let sub = app.db.setting_or("access.fx.sub", "");
            if token.is_empty() {
                app.access.set(&app, gen, "needs_login", Some("Войдите в fxTunnel — это бесплатно".into()), Some(None));
                return;
            }
            if sub.is_empty() {
                app.access.set(&app, gen, "error", Some("Выберите адрес сайта".into()), Some(None));
                return;
            }
            app.access.set(&app, gen, "starting", Some("Подключаемся к fxTunnel…".into()), None);
            let err = match Self::run_once(&app, gen, &token, &sub).await {
                Ok(e) => e,
                Err(e) => e.to_string(),
            };
            if !app.access.current(gen) {
                return;
            }
            if AUTH_ERROR.is_match(&err) {
                let _ = app.db.set_setting("access.fx.token", "");
                app.access.set(
                    &app,
                    gen,
                    "needs_login",
                    Some("Вход в fxTunnel устарел — войдите заново".into()),
                    Some(None),
                );
                return;
            }
            let msg = if TAKEN_ERROR.is_match(&err) {
                failures = BACKOFF.len() - 1;
                "Адрес занят другим пользователем fxTunnel. Выберите другой".to_string()
            } else if LIMIT_ERROR.is_match(&err) {
                "С этого аккаунта fxTunnel уже открыт другой туннель. Закройте его — подключимся сами".to_string()
            } else if err.is_empty() {
                "Связь с fxTunnel прервалась — переподключаемся".to_string()
            } else {
                format!("fxTunnel: {err} — переподключаемся")
            };
            app.access.set(&app, gen, "retrying", Some(msg), Some(None));
            let delay = BACKOFF[failures.min(BACKOFF.len() - 1)];
            failures += 1;
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(delay)) => {}
                _ = app.access.stop.notified() => {}
            }
            if !app.access.current(gen) {
                return;
            }
        }
    }

    /// Один запуск клиента; возвращает последнюю ошибку, когда клиент завершился.
    async fn run_once(app: &Arc<App>, gen: u64, token: &str, sub: &str) -> anyhow::Result<String> {
        let bin = Self::ensure_client(app, gen).await?;
        if !app.access.current(gen) {
            return Ok(String::new());
        }
        let pid_file = app.data.join("tools/fxtunnel/run.pid");
        kill_stale(&pid_file, "fxtunnel");
        let mut child = crate::library::probe::command(&bin)
            .args(["http", &app.ports.0.to_string(), "--domain", sub, "--no-inspect"])
            .env("FXTUNNEL_TOKEN", token)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(pid) = child.id() {
            let _ = std::fs::write(&pid_file, pid.to_string());
        }
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        for out in [
            child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
            child.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    let _ = tx.send(ANSI.replace_all(&l, "").trim().to_string());
                }
            });
        }
        drop(tx);
        let mut last_err = String::new();
        let mut online = false;
        let mut warned = false;
        let slow = tokio::time::Instant::now() + Duration::from_secs(45);
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        loop {
            tokio::select! {
                line = rx.recv() => match line {
                    Some(l) => {
                        if let Some(c) = HTTPS_URL.captures(&l) {
                            online = true;
                            app.access.set(app, gen, "online", None, Some(Some(c[1].trim_end_matches('/').to_string())));
                        } else if let Some(c) = FAILED.captures(&l) {
                            last_err = c[1].to_string();
                        } else if l.to_lowercase().contains("error") {
                            last_err = l.chars().take(200).collect();
                        }
                    }
                    None => break,
                },
                _ = app.access.stop.notified() => {}
                _ = tick.tick() => {}
                _ = tokio::time::sleep_until(slow), if !online && !warned => {
                    warned = true;
                    app.access.set(app, gen, "starting", Some("fxTunnel отвечает медленно — ждём".into()), None);
                }
            }
            if !app.access.current(gen) {
                let _ = child.kill().await;
                let _ = std::fs::remove_file(&pid_file);
                return Ok(String::new());
            }
        }
        let _ = child.wait().await;
        let _ = std::fs::remove_file(&pid_file);
        Ok(last_err)
    }

    async fn ensure_client(app: &Arc<App>, gen: u64) -> anyhow::Result<PathBuf> {
        let key = platform().ok_or_else(|| anyhow::anyhow!("для этой системы нет клиента fxTunnel"))?;
        let sha = FX_SHA256
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, s)| *s)
            .ok_or_else(|| anyhow::anyhow!("нет клиента для {key}"))?;
        let dir = app.data.join("tools/fxtunnel");
        let target = dir.join(if cfg!(windows) { "fxtunnel.exe" } else { "fxtunnel" });
        if target.is_file() {
            return Ok(target);
        }
        app.access.set(app, gen, "starting", Some("Скачиваем клиент fxTunnel…".into()), None);
        std::fs::create_dir_all(&dir)?;
        let asset = format!("fxtunnel-{key}{}", if cfg!(windows) { ".exe" } else { "" });
        let url = format!("https://github.com/mephistofox/fxtun.dev/releases/download/{FX_VERSION}/{asset}");
        let bytes =
            app.http.get(url).timeout(Duration::from_secs(300)).send().await?.error_for_status()?.bytes().await?;
        if hex::encode(Sha256::digest(&bytes)) != sha {
            anyhow::bail!("скачанный клиент fxTunnel не совпал с ожидаемым — не запускаем");
        }
        let part = dir.join(format!("{asset}.part"));
        std::fs::write(&part, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755))?;
        }
        std::fs::rename(&part, &target)?;
        Ok(target)
    }

    // ---------- вход в fxTunnel через браузер ----------

    pub async fn start_login(app: &Arc<App>) -> anyhow::Result<LoginView> {
        let v: serde_json::Value = app
            .http
            .post(format!("{FX_API}/api/auth/device/code"))
            .header("Accept", "application/json")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let code = v["user_code"]
            .as_str()
            .filter(|c| !c.is_empty())
            .ok_or_else(|| anyhow::anyhow!("fxTunnel не выдал код входа"))?;
        let session = v["session_id"].as_str().unwrap_or_default().to_string();
        let lv = LoginView {
            code: code.to_string(),
            url: v["auth_url"].as_str().unwrap_or(FX_API).to_string(),
            expires_at: now_ms() + v["expires_in"].as_i64().unwrap_or(600) * 1000,
        };
        app.access.inner.lock().login = Some(lv.clone());
        let a = app.clone();
        let expires = lv.expires_at;
        tokio::spawn(async move {
            while now_ms() < expires {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let r =
                    a.http.get(format!("{FX_API}/api/auth/device/token")).query(&[("session", &session)]).send().await;
                let Ok(r) = r else { continue };
                let Ok(j) = r.json::<serde_json::Value>().await else {
                    continue;
                };
                match j["status"].as_str() {
                    Some("authorized") => {
                        if let Some(t) = j["token"].as_str() {
                            let _ = a.db.set_setting("access.fx.token", t);
                            if a.db.setting_or("access.fx.sub", "").is_empty() {
                                let _ = a.db.set_setting("access.fx.sub", &suggest_subdomain());
                            }
                        }
                        a.access.inner.lock().login = None;
                        let _ = a.db.set_setting("access.mode", "fxtunnel");
                        Access::restart(&a);
                        return;
                    }
                    Some("expired") => break,
                    _ => {}
                }
            }
            a.access.inner.lock().login = None;
        });
        Ok(lv)
    }

    pub fn logout(app: &Arc<App>) {
        let _ = app.db.set_setting("access.fx.token", "");
        Access::restart(app);
    }
}

pub fn suggest_subdomain() -> String {
    format!("kino-{}", crate::util::human_code(5).to_lowercase())
}

/// Клиент туннеля с прошлого запуска (после аварийного выхода) занимает адрес — останавливаем,
/// если это действительно он.
fn kill_stale(pid_file: &std::path::Path, name: &str) {
    let Ok(s) = std::fs::read_to_string(pid_file) else {
        return;
    };
    let Ok(pid) = s.trim().parse::<u32>() else {
        return;
    };
    let _ = std::fs::remove_file(pid_file);
    #[cfg(unix)]
    {
        let out = std::process::Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output();
        if out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(name)) {
            let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .creation_flags(0x0800_0000)
            .output();
        if out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains(name)) {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .creation_flags(0x0800_0000)
                .status();
        }
    }
}

impl Access {
    /// Остановить туннель при выходе из программы.
    pub fn shutdown(&self) {
        self.inner.lock().gen += 1;
        self.stop.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_client_output() {
        let l = ANSI.replace_all("\u{1b}[32m  HTTPS: https://kino-ab12.fxtun.ru\u{1b}[0m", "").trim().to_string();
        assert_eq!(&HTTPS_URL.captures(&l).unwrap()[1], "https://kino-ab12.fxtun.ru");
        assert!(AUTH_ERROR.is_match("error: invalid token"));
        assert!(TAKEN_ERROR.is_match("subdomain is already taken"));
        assert!(SUBDOMAIN.is_match(&suggest_subdomain()));
        assert!(!SUBDOMAIN.is_match("-bad"));
    }
}
