//! Nova (внутреннее имя — fsfy, бывший «Films & Serials For You») — домашний кинотеатр: сервер раздаёт фильмы и сериалы с компьютера
//! хоста, интерфейс — в браузере у зрителей и в окне приложения у хоста.

pub mod access;
pub mod api;
pub mod auth;
pub mod cloudpub;
pub mod copy;
pub mod db;
pub mod discover;
pub mod error;
pub mod hls;
pub mod host;
pub mod images;
pub mod library;
pub mod meta;
pub mod release;
pub mod stream;
pub mod taste;
pub mod util;
pub mod web;

use anyhow::Context;
use axum::Extension;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Notify, Semaphore};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = "Nova";
pub const DEFAULT_PORT: u16 = 17480;
pub const DEFAULT_WINDOW_PORT: u16 = 17481;

pub struct App {
    pub db: db::Db,
    pub data: PathBuf,
    pub images: images::Images,
    pub http: reqwest::Client,
    pub tools: RwLock<library::probe::Tools>,
    pub host: Arc<dyn host::Host>,
    /// (общий порт, порт окна хоста)
    pub ports: (u16, u16),
    pub scan_kick: Notify,
    pub scan_state: Mutex<library::jobs::ScanState>,
    pub meta_kick: Notify,
    pub meta_state: Mutex<meta::MetaState>,
    pub probe_kick: Notify,
    /// Лента IMDb: обновить подборки / докачать постеры.
    pub discover_kick: Notify,
    pub posters_kick: Notify,
    pub access: access::Access,
    pub login_limit: auth::Limiter,
    pub signup_limit: auth::Limiter,
    /// Сообщений от профиля — не больше 30 в минуту.
    pub message_limit: auth::Limiter,
    pub entries: Mutex<HashMap<String, i64>>,
    /// Перекодирование нагружает процессор — одновременно немного.
    pub transcode: Arc<Semaphore>,
    pub remux: Arc<Semaphore>,
    pub hls: hls::Hls,
    pub copies: copy::Copies,
    pub inbox: library::inbox::Inbox,
}

impl App {
    pub fn kick_probe(&self) {
        self.probe_kick.notify_one();
    }

    pub fn reload_tools(&self) {
        let configured = self.db.setting("tools.ffmpeg");
        *self.tools.write() = library::probe::Tools::find(configured.as_deref());
    }

    /// Одноразовая ссылка входа для окна хоста (2 минуты).
    pub fn window_entry(&self) -> String {
        let t = util::token(24);
        let now = util::now_ms();
        let mut e = self.entries.lock();
        e.retain(|_, exp| *exp > now);
        e.insert(t.clone(), now + 120_000);
        format!("http://127.0.0.1:{}/api/host/enter?t={t}", self.ports.1)
    }

    pub fn site_name(&self) -> String {
        self.db.setting_or("site.name", NAME)
    }
}

pub struct Options {
    pub data: PathBuf,
    pub port: Option<u16>,
    pub window_port: Option<u16>,
}

pub fn default_data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("FSFY_DATA") {
        return PathBuf::from(d);
    }
    let home =
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/app.fsfy")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or(home).join("app.fsfy")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("app.fsfy")
    }
}

/// Занять порт; занят — соседний, потом любой. Выбранный запоминается: у окна хоста от порта
/// зависят сохранённые настройки браузера.
async fn bind(ip: Ipv4Addr, wanted: u16) -> anyhow::Result<TcpListener> {
    for p in (wanted..wanted.saturating_add(20)).step_by(2) {
        if let Ok(l) = TcpListener::bind((ip, p)).await {
            return Ok(l);
        }
    }
    Ok(TcpListener::bind((ip, 0)).await?)
}

pub struct Running {
    pub app: Arc<App>,
    stop: tokio::sync::watch::Sender<bool>,
    servers: Vec<tokio::task::JoinHandle<()>>,
}

impl Running {
    pub fn entry_url(&self) -> String {
        self.app.window_entry()
    }

    /// Остановить приём запросов и дождаться окончания.
    pub async fn shutdown(self) {
        let _ = self.stop.send(true);
        self.app.access.shutdown();
        for s in self.servers {
            let _ = tokio::time::timeout(Duration::from_secs(5), s).await;
        }
    }

    pub async fn wait(self) {
        for s in self.servers {
            let _ = s.await;
        }
    }
}

pub async fn start(opts: Options, host: Arc<dyn host::Host>) -> anyhow::Result<Running> {
    std::fs::create_dir_all(&opts.data).with_context(|| format!("каталог данных {}", opts.data.display()))?;
    let db = db::Db::open(&opts.data.join("fsfy.db"))?;
    let images = images::Images::new(opts.data.join("images"))?;
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (compatible; fsfy/".to_string() + VERSION + ")")
        .build()?;

    let port = opts.port.or_else(|| db.setting("net.port").and_then(|p| p.parse().ok())).unwrap_or(DEFAULT_PORT);
    let wport = opts
        .window_port
        .or_else(|| db.setting("net.window_port").and_then(|p| p.parse().ok()))
        .unwrap_or(DEFAULT_WINDOW_PORT);
    let public = bind(Ipv4Addr::UNSPECIFIED, port).await?;
    let window = bind(Ipv4Addr::LOCALHOST, wport).await?;
    let ports = (public.local_addr()?.port(), window.local_addr()?.port());
    if opts.port.is_none() {
        db.set_setting("net.port", &ports.0.to_string())?;
    }
    if opts.window_port.is_none() {
        db.set_setting("net.window_port", &ports.1.to_string())?;
    }

    let app = Arc::new(App {
        tools: RwLock::new(library::probe::Tools::find(db.setting("tools.ffmpeg").as_deref())),
        db,
        data: opts.data.clone(),
        images,
        http,
        host,
        ports,
        scan_kick: Notify::new(),
        scan_state: Mutex::default(),
        meta_kick: Notify::new(),
        meta_state: Mutex::default(),
        probe_kick: Notify::new(),
        discover_kick: Notify::new(),
        posters_kick: Notify::new(),
        access: access::Access::default(),
        login_limit: auth::Limiter::new(10, 10 * 60 * 1000),
        signup_limit: auth::Limiter::new(5, 10 * 60 * 1000),
        message_limit: auth::Limiter::new(30, 60 * 1000),
        entries: Mutex::default(),
        transcode: Arc::new(Semaphore::new(2)),
        remux: Arc::new(Semaphore::new(8)),
        hls: hls::Hls::default(),
        copies: copy::Copies::default(),
        inbox: library::inbox::Inbox::default(),
    });
    auth::ensure_host(&app)?;
    backfill(&app)?;
    library::inbox::ensure_on_start(&app);

    tokio::spawn(library::jobs::scan_worker(app.clone()));
    tokio::task::spawn_blocking(web::warm);
    tokio::spawn(library::jobs::probe_worker(app.clone()));
    tokio::spawn(library::inbox::watch_worker(app.clone()));
    tokio::spawn(meta::worker(app.clone()));
    tokio::spawn(cleanup(app.clone()));
    tokio::spawn(hls::janitor(app.clone()));
    tokio::spawn(copy::janitor(app.clone()));
    tokio::spawn(discover::worker(app.clone()));
    tokio::spawn(discover::posters(app.clone()));
    access::Access::restart(&app);

    let router = api::router(app.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let mut servers = Vec::new();
    for (listener, window) in [(public, false), (window, true)] {
        let r = router.clone().layer(Extension(auth::Via { window }));
        let mut rx = rx.clone();
        servers.push(tokio::spawn(async move {
            let res = axum::serve(listener, r.into_make_service_with_connect_info::<SocketAddr>())
                .with_graceful_shutdown(async move {
                    let _ = rx.wait_for(|s| *s).await;
                })
                .await;
            if let Err(e) = res {
                tracing::error!(error = %e, "сервер остановился");
            }
        }));
    }
    tracing::info!(port = ports.0, window_port = ports.1, "сервер запущен");
    Ok(Running { app, stop, servers })
}

/// Данные, появившиеся в новой версии, для старых записей.
fn backfill(app: &App) -> rusqlite::Result<()> {
    let ages: Vec<(i64, String)> = app.db.with(|c| {
        let mut st = c.prepare("SELECT id, age FROM titles WHERE age IS NOT NULL AND age_min IS NULL")?;
        let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        r.collect()
    })?;
    app.db.tx(|tx| {
        for (id, age) in ages {
            tx.execute("UPDATE titles SET age_min = ? WHERE id = ?", rusqlite::params![taste::age_min(&age), id])?;
        }
        Ok(())
    })
}

/// Раз в час: просроченные сессии и приглашения.
async fn cleanup(app: Arc<App>) {
    loop {
        let now = util::now_ms();
        let _ = app.db.with(|c| {
            c.execute("DELETE FROM sessions WHERE expires_at < ?", [now])?;
            c.execute("DELETE FROM invites WHERE expires_at IS NOT NULL AND expires_at < ?", [now - 7 * 86_400_000])
        });
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
