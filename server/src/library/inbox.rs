//! Добавить фильмы без лишних шагов:
//! - **своя папка** «Films & Serials» (в «Фильмах»/«Видео» хоста) создаётся и подключается сама —
//!   достаточно положить туда файлы;
//! - **перетащить** в окно приложения: папка подключается как есть (без копирования), отдельные
//!   файлы копируются в свою папку (с прогрессом);
//! - **загрузить** из браузера с другого устройства (хост или администратор);
//! - **слежение**: изменения в папках подхватываются сами за несколько секунд.

use crate::util::now_ms;
use crate::App;
use notify::{RecursiveMode, Watcher};
use parking_lot::Mutex;
use rusqlite::params;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub const FOLDER: &str = "Films & Serials";
const SUBS_EXT: &[&str] = &["srt", "ass", "ssa", "vtt", "sub"];

/// Копирование перетащенных файлов: сколько всего и сколько готово.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportState {
    pub running: bool,
    pub files_total: usize,
    pub files_done: usize,
    pub bytes_total: u64,
    pub bytes_done: u64,
    pub current: Option<String>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Inbox {
    pub state: Mutex<ImportState>,
    queue: Mutex<Vec<PathBuf>>,
}

/// Что можно класть в библиотеку: видео и субтитры.
pub fn accepted(path: &Path) -> bool {
    super::parse::is_video(path)
        || path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| SUBS_EXT.contains(&e.to_ascii_lowercase().as_str()))
}

/// Имя файла или папки из чужого ввода: только последняя часть, без служебных символов.
pub fn safe_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?.trim();
    let clean: String = base
        .chars()
        .map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') { '_' } else { c })
        .collect();
    let clean = clean.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    (!clean.is_empty() && clean.chars().count() <= 200).then_some(clean)
}

/// Своя папка: из настроек; нет — «Фильмы» хоста или `<данные>/media`. Создаётся и
/// подключается к библиотеке, если ещё нет.
pub fn folder(app: &App) -> anyhow::Result<PathBuf> {
    let dir = match app.db.setting("media.inbox").map(PathBuf::from) {
        Some(p) => p,
        None => app.host.media_dir().map(|d| d.join(FOLDER)).unwrap_or_else(|| app.data.join("media")),
    };
    std::fs::create_dir_all(&dir)?;
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    let s = dir.to_string_lossy().to_string();
    app.db.set_setting("media.inbox", &s)?;
    let added = app.db.with(|c| {
        c.execute("INSERT OR IGNORE INTO libraries (path, kind, added_at) VALUES (?, 'auto', ?)", params![s, now_ms()])
    })?;
    if added > 0 {
        app.scan_kick.notify_one();
    }
    Ok(dir)
}

/// При запуске в приложении хоста: своя папка есть всегда (в `fsfy serve` — только по запросу).
pub fn ensure_on_start(app: &App) {
    if app.host.media_dir().is_some() || app.db.setting("media.inbox").is_some() {
        if let Err(e) = folder(app) {
            tracing::warn!(error = %e, "своя папка для фильмов");
        }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dropped {
    /// Папки, подключённые как есть.
    pub folders: Vec<String>,
    /// Файлы в очереди на копирование.
    pub files: usize,
    /// Пропущено: не видео и уже подключённое.
    pub skipped: usize,
}

/// Перетащили в окно: папки — в библиотеку, файлы — копировать в свою папку.
pub fn import(app: &Arc<App>, paths: Vec<PathBuf>) -> Dropped {
    let mut out = Dropped::default();
    let libraries: Vec<PathBuf> = app
        .db
        .with(|c| {
            let mut st = c.prepare("SELECT path FROM libraries")?;
            let r = st.query_map([], |r| r.get::<_, String>(0))?;
            r.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let inside = |p: &Path| libraries.iter().any(|l| p.starts_with(l));
    let mut files = Vec::new();
    for p in paths {
        let p = std::fs::canonicalize(&p).unwrap_or(p);
        if inside(&p) {
            out.skipped += 1;
        } else if p.is_dir() {
            let s = p.to_string_lossy().to_string();
            let _ = app.db.with(|c| {
                c.execute(
                    "INSERT OR IGNORE INTO libraries (path, kind, added_at) VALUES (?, 'auto', ?)",
                    params![s, now_ms()],
                )
            });
            out.folders.push(s);
        } else if p.is_file() && accepted(&p) {
            files.push(p);
        } else {
            out.skipped += 1;
        }
    }
    if !out.folders.is_empty() {
        app.scan_kick.notify_one();
    }
    out.files = files.len();
    if !files.is_empty() {
        let start = {
            let mut q = app.inbox.queue.lock();
            let idle = q.is_empty() && !app.inbox.state.lock().running;
            q.extend(files);
            idle
        };
        if start {
            let a = app.clone();
            tokio::task::spawn_blocking(move || copy_queue(&a));
        }
    }
    out
}

/// Копирует очередь в свою папку: сначала `*.part`, потом переименование — сканер не видит
/// недокопированное. Файл с таким именем уже есть — не трогаем.
fn copy_queue(app: &Arc<App>) {
    let dir = match folder(app) {
        Ok(d) => d,
        Err(e) => {
            app.inbox.state.lock().error = Some(e.to_string());
            return;
        }
    };
    {
        let q = app.inbox.queue.lock();
        let mut st = app.inbox.state.lock();
        *st = ImportState {
            running: true,
            files_total: q.len(),
            bytes_total: q.iter().filter_map(|p| p.metadata().ok()).map(|m| m.len()).sum(),
            ..Default::default()
        };
    }
    loop {
        let next = {
            let mut q = app.inbox.queue.lock();
            if q.is_empty() {
                None
            } else {
                Some(q.remove(0))
            }
        };
        let Some(src) = next else { break };
        let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        app.inbox.state.lock().current = Some(name.clone());
        let dst = dir.join(&name);
        if !dst.exists() {
            if let Err(e) = copy_one(app, &src, &dst) {
                tracing::warn!(error = %e, "копирование");
                app.inbox.state.lock().error = Some(format!("{name}: {e}"));
            }
        }
        let mut st = app.inbox.state.lock();
        st.files_done += 1;
        // Добавили ещё, пока копировали.
        st.files_total = st.files_total.max(st.files_done + app.inbox.queue.lock().len());
    }
    let mut st = app.inbox.state.lock();
    st.running = false;
    st.current = None;
    drop(st);
    app.scan_kick.notify_one();
}

fn copy_one(app: &App, src: &Path, dst: &Path) -> std::io::Result<()> {
    let part = dst.with_extension(format!("{}.part", dst.extension().and_then(|e| e.to_str()).unwrap_or("")));
    let mut from = std::fs::File::open(src)?;
    let mut to = std::fs::File::create(&part)?;
    let mut buf = vec![0u8; 8 << 20];
    let res = (|| {
        loop {
            let n = from.read(&mut buf)?;
            if n == 0 {
                break;
            }
            to.write_all(&buf[..n])?;
            app.inbox.state.lock().bytes_done += n as u64;
        }
        to.sync_all()
    })();
    drop(to);
    match res {
        Ok(()) => std::fs::rename(&part, dst),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

/// Загрузка из браузера: куда положить (своя папка / подпапка).
pub fn upload_target(app: &App, name: &str, subfolder: Option<&str>) -> Result<(PathBuf, PathBuf), String> {
    let name = safe_name(name).ok_or("Неверное имя файла")?;
    if !accepted(Path::new(&name)) {
        return Err("Можно загрузить видео и субтитры".into());
    }
    let mut dir = folder(app).map_err(|e| e.to_string())?;
    // Папка — с вложенными («Сериал/Сезон 1»), как её выбрали: каждая часть проверяется отдельно.
    if let Some(s) = subfolder.filter(|s| !s.trim().is_empty()) {
        let parts: Vec<&str> = s.split(['/', '\\']).filter(|p| !p.trim().is_empty()).collect();
        if parts.len() > 4 {
            return Err("Слишком глубокая папка".into());
        }
        for p in parts {
            if p.trim().trim_matches('.').is_empty() {
                return Err("Неверное имя папки".into());
            }
            dir = dir.join(safe_name(p).ok_or("Неверное имя папки")?);
        }
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let dst = dir.join(&name);
    if dst.exists() {
        return Err(format!("«{name}» уже есть"));
    }
    let part = dir.join(format!("{name}.part"));
    Ok((part, dst))
}

/// Слежение за папками библиотеки: изменились — пересканировать через 3 с тишины. Список папок
/// перечитывается раз в 5 с (добавили или убрали папку).
/// Появилось, пропало, переименовано или изменилось содержимое — повод сканировать.
fn changes_files(kind: &notify::EventKind) -> bool {
    use notify::event::{AccessKind, AccessMode, ModifyKind};
    use notify::EventKind;
    match kind {
        EventKind::Create(_) | EventKind::Remove(_) => true,
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Modify(m) => !matches!(m, ModifyKind::Metadata(_)),
        _ => false,
    }
}

pub async fn watch_worker(app: Arc<App>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let mut watched: Vec<String> = Vec::new();
    // Держим живым: удалили — слежение прекратилось.
    let mut _watcher: Option<notify::RecommendedWatcher> = None;
    loop {
        let libs: Vec<String> = app
            .db
            .with(|c| {
                let mut st = c.prepare("SELECT path FROM libraries ORDER BY id")?;
                let r = st.query_map([], |r| r.get::<_, String>(0))?;
                r.collect()
            })
            .unwrap_or_default();
        if libs != watched {
            let t = tx.clone();
            let mut w = notify::recommended_watcher(move |e: notify::Result<notify::Event>| {
                if let Ok(e) = e {
                    // Свои временные файлы (*.part) — не повод сканировать. Чтение и атрибуты —
                    // тоже: сам скан читает папку, и на Linux (inotify) сканы шли по кругу раз в 3 с.
                    if changes_files(&e.kind) && e.paths.iter().any(|p| p.extension().is_none_or(|x| x != "part")) {
                        let _ = t.send(());
                    }
                }
            })
            .map_err(|e| tracing::warn!(error = %e, "слежение за папками недоступно"))
            .ok();
            if let Some(w) = w.as_mut() {
                for l in &libs {
                    if let Err(e) = w.watch(Path::new(l), RecursiveMode::Recursive) {
                        tracing::debug!(error = %e, path = l, "папку не отследить");
                    }
                }
            }
            _watcher = w;
            watched = libs;
        }
        tokio::select! {
            got = rx.recv() => {
                if got.is_none() { return; }
                // Ждём тишины: копирование большого файла даёт много событий.
                while let Ok(Some(())) = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {}
                app.scan_kick.notify_one();
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_changes_trigger_scan() {
        use notify::event::{AccessKind, CreateKind, DataChange, MetadataKind, ModifyKind, RenameMode};
        use notify::EventKind;
        assert!(changes_files(&EventKind::Create(CreateKind::File)));
        assert!(changes_files(&EventKind::Modify(ModifyKind::Data(DataChange::Any))));
        assert!(changes_files(&EventKind::Modify(ModifyKind::Name(RenameMode::Both))));
        assert!(changes_files(&EventKind::Access(AccessKind::Close(notify::event::AccessMode::Write))));
        assert!(!changes_files(&EventKind::Access(AccessKind::Read)));
        assert!(!changes_files(&EventKind::Access(AccessKind::Open(notify::event::AccessMode::Any))));
        assert!(!changes_files(&EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime))));
    }

    #[test]
    fn names_and_types() {
        assert_eq!(safe_name("../../etc/passwd").as_deref(), Some("passwd"));
        assert_eq!(safe_name("C:\\Users\\a\\Фильм (2020).mkv").as_deref(), Some("Фильм (2020).mkv"));
        assert_eq!(safe_name("bad:name?.mp4").as_deref(), Some("bad_name_.mp4"));
        assert_eq!(safe_name(" .. "), None);
        assert_eq!(safe_name(""), None);
        assert!(accepted(Path::new("a.MKV")));
        assert!(accepted(Path::new("a.ru.srt")));
        assert!(!accepted(Path::new("a.exe")));
        assert!(!accepted(Path::new("poster.jpg")));
    }
}
