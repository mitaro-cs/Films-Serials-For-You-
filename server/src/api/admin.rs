//! Управление (хост и администраторы): папки, описания, люди, приглашения, доступ, настройки.

use super::img;
use crate::access::{Access, SUBDOMAIN};
use crate::auth::{self, Manager};
use crate::error::{ApiError, ApiResult};
use crate::meta::{self, Provider};
use crate::util::{human_code, now_ms};
use crate::App;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

type Out = ApiResult<Json<Value>>;

fn ok() -> Out {
    Ok(Json(json!({"ok": true})))
}

pub async fn status(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let (titles, files, size, pending, unprobed): (i64, i64, i64, i64, i64) = app.db.with(|c| {
        c.query_row(
            "SELECT (SELECT COUNT(*) FROM titles t WHERE EXISTS (SELECT 1 FROM files f WHERE f.title_id = t.id AND f.missing = 0)),
                    (SELECT COUNT(*) FROM files WHERE missing = 0),
                    (SELECT COALESCE(SUM(size), 0) FROM files WHERE missing = 0),
                    (SELECT COUNT(*) FROM titles WHERE meta_status = 'pending' AND locked = 0),
                    (SELECT COUNT(*) FROM files WHERE probe IS NULL AND missing = 0)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
    })?;
    // «Сейчас смотрят»: прогресс за последнюю минуту (плеер шлёт его раз в 10 с).
    let watching: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT CASE WHEN pr.name = u.name THEN u.name ELSE u.name || ' · ' || pr.name END,
                    t.title, f.season, f.episode, p.position, p.duration, p.profile_id FROM progress p
             JOIN profiles pr ON pr.id = p.profile_id JOIN users u ON u.id = pr.user_id
             JOIN files f ON f.id = p.file_id JOIN titles t ON t.id = f.title_id
             WHERE p.updated_at > ? ORDER BY p.updated_at DESC",
        )?;
        let net = app.playback.lock().clone();
        let r = st.query_map([now_ms() - 60_000], |r| {
            // Отчёт плеера за последние полторы минуты: скорость, качество, остановки.
            let stat = net.get(&r.get::<_, i64>(6)?).filter(|s| now_ms() - s.at < 90_000);
            Ok(json!({
                "user": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?,
                "episode": super::catalog::episode_label(r.get(2)?, r.get(3)?, None),
                "position": r.get::<_, f64>(4)?, "duration": r.get::<_, f64>(5)?,
                "net": stat,
            }))
        })?;
        r.collect()
    })?;
    let tools = app.tools.read().clone();
    Ok(Json(json!({
        "version": crate::VERSION,
        "system": crate::monitor::snapshot(&app.data),
        "update": app.host.update_info(),
        "ports": {"public": app.ports.0, "window": app.ports.1},
        "titles": titles, "files": files, "size": size,
        "metaPending": pending, "unprobed": unprobed,
        "scan": *app.scan_state.lock(),
        "meta": *app.meta_state.lock(),
        "ffmpeg": tools.ffmpeg.is_some(), "ffprobe": tools.ffprobe.is_some(),
        "watching": watching,
        "canPick": app.host.can_pick(),
        "publicUrl": app.access.public_url(&app),
        "inbox": app.db.setting("media.inbox"),
        "import": *app.inbox.state.lock(),
    })))
}

/// Своя папка для фильмов: создать (если нет) и подключить.
pub async fn inbox(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let dir = crate::library::inbox::folder(&app).map_err(|e| ApiError::bad("inbox", e.to_string()))?;
    Ok(Json(json!({"path": dir.display().to_string()})))
}

/// Открыть свою папку в Finder/Проводнике - только в окне на компьютере хоста.
pub async fn reveal_inbox(State(app): State<Arc<App>>, m: Manager) -> Out {
    if !m.0.local {
        return Err(ApiError::bad("not_local", "Открыть папку можно только на компьютере хоста"));
    }
    let dir = crate::library::inbox::folder(&app).map_err(|e| ApiError::bad("inbox", e.to_string()))?;
    Ok(Json(json!({"ok": app.host.reveal(&dir), "path": dir.display().to_string()})))
}

#[derive(Deserialize)]
pub struct UploadQuery {
    name: String,
    folder: Option<String>,
    /// Впереди ещё файлы этой загрузки: обход библиотеки - после последнего.
    #[serde(default)]
    more: u8,
}

/// Загрузка файла из браузера в свою папку: потоком на диск (`*.part`), в конце - переименование.
pub async fn upload(
    State(app): State<Arc<App>>,
    _m: Manager,
    Query(q): Query<UploadQuery>,
    body: axum::body::Body,
) -> Out {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let (part, dst) = crate::library::inbox::upload_target(&app, &q.name, q.folder.as_deref())
        .map_err(|e| ApiError::bad("bad_upload", e))?;
    let mut file = tokio::fs::File::create(&part).await?;
    let mut stream = body.into_data_stream();
    let mut size: u64 = 0;
    let res: Result<(), String> = async {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("связь прервалась: {e}"))?;
            size += chunk.len() as u64;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.sync_all().await.map_err(|e| e.to_string())
    }
    .await;
    drop(file);
    if let Err(e) = res.and_then(|_| if size == 0 { Err("пустой файл".into()) } else { Ok(()) }) {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(ApiError::bad("upload_failed", format!("Не загрузилось: {e}")));
    }
    tokio::fs::rename(&part, &dst).await?;
    if q.more == 0 {
        app.scan_kick.notify_one();
    }
    Ok(Json(json!({"ok": true, "size": size, "path": dst.display().to_string()})))
}

/// «Проверить обновления» и «Обновить» из окна приложения: только хост и только в приложении.
pub async fn update(State(app): State<Arc<App>>, m: Manager, Path(what): Path<String>) -> Out {
    if m.0.role != "host" {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "host_only", "Обновляет только хост"));
    }
    if app.host.update_info().is_none() {
        return Err(ApiError::bad("no_updater", "Сервер запущен без приложения - обновите его вручную"));
    }
    match what.as_str() {
        "check" => app.host.update_check(),
        "install" => app.host.update_install(),
        _ => return Err(ApiError::not_found()),
    }
    Ok(Json(json!({"ok": true, "update": app.host.update_info()})))
}

pub async fn settings(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let db = &app.db;
    let tools = app.tools.read().clone();
    Ok(Json(json!({
        "siteName": app.site_name(),
        "metadataLanguage": db.setting_or("meta.language", "ru"),
        "registration": db.setting_or("site.registration", crate::api::account::REGISTRATION_DEFAULT),
        "downloads": db.setting_or("site.downloads", "1") == "1",
        "manageRemote": db.setting_or("security.manage_remote", "0") == "1",
        "provider": db.setting_or("meta.provider", "imdb"),
        "tmdbKeySet": !db.setting_or("meta.tmdb.key", "").is_empty(),
        "kpKeySet": !db.setting_or("meta.kp.key", "").is_empty(),
        "vkTokenSet": !db.setting_or("vk.token", "").is_empty(),
        "sites": super::links::extra_sites(&app),
        "tmdbApi": db.setting_or("meta.tmdb.api", ""),
        "tmdbImages": db.setting_or("meta.tmdb.images", ""),
        "ffmpegPath": db.setting_or("tools.ffmpeg", ""),
        "ffmpeg": tools.ffmpeg.map(|p| p.display().to_string()),
        "ffprobe": tools.ffprobe.map(|p| p.display().to_string()),
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsBody {
    metadata_language: Option<String>,
    site_name: Option<String>,
    registration: Option<String>,
    downloads: Option<bool>,
    /// Управление не только из окна приложения (только хост, только с входом в два шага).
    manage_remote: Option<bool>,
    provider: Option<String>,
    tmdb_key: Option<String>,
    kp_key: Option<String>,
    tmdb_api: Option<String>,
    tmdb_images: Option<String>,
    ffmpeg_path: Option<String>,
    vk_token: Option<String>,
    /// Свои видеосайты хоста (домены).
    sites: Option<Vec<String>>,
}

fn url_or_empty(s: &str) -> ApiResult<String> {
    let s = s.trim().trim_end_matches('/');
    if s.is_empty() || s.starts_with("https://") || s.starts_with("http://") {
        Ok(s.to_string())
    } else {
        Err(ApiError::bad("bad_url", "Адрес должен начинаться с https://"))
    }
}

pub async fn save_settings(State(app): State<Arc<App>>, m: Manager, Json(b): Json<SettingsBody>) -> Out {
    let db = &app.db;
    if let Some(language) = b.metadata_language {
        if !matches!(language.as_str(), "ru" | "source") {
            return Err(ApiError::bad("language", "Неизвестный язык описаний"));
        }
        db.set_setting("meta.language", &language)?;
    }
    // Путь к ffmpeg — это программа, которую сервер запустит: менять его может только хост из
    // своего окна, и только на локальный диск (не \\сервер\папка — её содержимое чужое).
    if let Some(p) = b.ffmpeg_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        if m.0.role != "host" || !m.0.local {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "host_only",
                "Путь к ffmpeg меняет только хост в окне приложения",
            ));
        }
        if p.starts_with("\\\\") || p.starts_with("//") {
            return Err(ApiError::bad("bad_path", "Нужна папка на этом компьютере, а не в сети"));
        }
    }
    if let Some(n) = b.site_name {
        let n = n.trim();
        if n.is_empty() || n.chars().count() > 60 {
            return Err(ApiError::bad("bad_name", "Название - от 1 до 60 символов"));
        }
        db.set_setting("site.name", n)?;
    }
    if let Some(r) = b.registration {
        if !matches!(r.as_str(), "invite" | "open" | "closed") {
            return Err(ApiError::bad("bad_value", "Неизвестный режим регистрации"));
        }
        db.set_setting("site.registration", &r)?;
    }
    if let Some(r) = b.manage_remote {
        if m.0.role != "host" || !m.0.local {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "host_only", "Это меняет только хост в окне приложения"));
        }
        db.set_setting("security.manage_remote", if r { "1" } else { "0" })?;
    }
    if let Some(d) = b.downloads {
        db.set_setting("site.downloads", if d { "1" } else { "0" })?;
    }
    let mut meta_changed = false;
    for (key, val) in [("meta.tmdb.key", b.tmdb_key), ("meta.kp.key", b.kp_key)] {
        if let Some(v) = val {
            db.set_setting(key, v.trim())?;
            meta_changed = true;
        }
    }
    for (key, val) in [("meta.tmdb.api", b.tmdb_api), ("meta.tmdb.images", b.tmdb_images)] {
        if let Some(v) = val {
            db.set_setting(key, &url_or_empty(&v)?)?;
            meta_changed = true;
        }
    }
    if let Some(p) = b.provider {
        if !matches!(p.as_str(), "imdb" | "tmdb" | "kinopoisk" | "off") {
            return Err(ApiError::bad("bad_value", "Неизвестный источник описаний"));
        }
        meta_changed |= db.setting_or("meta.provider", "imdb") != p;
        db.set_setting("meta.provider", &p)?;
    }
    if meta_changed {
        // Новый источник или ключ: ещё раз ищем то, что раньше не нашлось.
        db.with(|c| c.execute("UPDATE titles SET meta_status = 'pending' WHERE meta_status = 'none'", []))?;
        app.meta_state.lock().error = None;
        app.meta_kick.notify_one();
    }
    if let Some(list) = b.sites {
        let mut keep: Vec<String> = Vec::new();
        for s in &list {
            let d = super::links::site_domain(s).ok_or_else(|| {
                ApiError::bad("bad_site", format!("«{s}» - не адрес сайта (нужно вроде kino.example)"))
            })?;
            if !keep.contains(&d) {
                keep.push(d);
            }
        }
        if keep.len() > 30 {
            return Err(ApiError::bad("too_many", "Своих сайтов - не больше 30"));
        }
        db.set_setting("video.sites", &serde_json::to_string(&keep).unwrap_or_else(|_| "[]".into()))?;
    }
    if let Some(t) = b.vk_token {
        db.set_setting("vk.token", t.trim())?;
    }
    if let Some(p) = b.ffmpeg_path {
        db.set_setting("tools.ffmpeg", p.trim())?;
        app.reload_tools();
        app.kick_probe();
    }
    ok()
}

pub async fn libraries(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let rows: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT l.id, l.path, l.kind, COUNT(f.id), COALESCE(SUM(f.size), 0) FROM libraries l
             LEFT JOIN files f ON f.library_id = l.id AND f.missing = 0 GROUP BY l.id ORDER BY l.id",
        )?;
        let r = st.query_map([], |r| {
            let path: String = r.get(1)?;
            Ok(json!({
                "id": r.get::<_, i64>(0)?, "exists": std::path::Path::new(&path).is_dir(), "path": path,
                "kind": r.get::<_, String>(2)?, "files": r.get::<_, i64>(3)?, "size": r.get::<_, i64>(4)?,
            }))
        })?;
        r.collect()
    })?;
    Ok(Json(json!(rows)))
}

#[derive(Deserialize)]
pub struct LibraryBody {
    path: String,
    kind: Option<String>,
}

pub async fn add_library(State(app): State<Arc<App>>, _m: Manager, Json(b): Json<LibraryBody>) -> Out {
    let kind = b.kind.unwrap_or_else(|| "auto".into());
    if !matches!(kind.as_str(), "auto" | "movies" | "series") {
        return Err(ApiError::bad("bad_value", "Неизвестный вид папки"));
    }
    let p = std::path::PathBuf::from(b.path.trim());
    if !p.is_absolute() || !p.is_dir() {
        return Err(ApiError::bad("bad_path", "Такой папки нет на этом компьютере"));
    }
    let p = std::fs::canonicalize(&p).unwrap_or(p);
    let path = p.to_str().ok_or_else(|| ApiError::bad("bad_path", "Неподдерживаемое имя папки"))?.to_string();
    let added = app.db.with(|c| {
        c.execute(
            "INSERT OR IGNORE INTO libraries (path, kind, added_at) VALUES (?, ?, ?)",
            params![path, kind, now_ms()],
        )
    })?;
    if added == 0 {
        return Err(ApiError::new(StatusCode::CONFLICT, "exists", "Эта папка уже добавлена"));
    }
    app.scan_kick.notify_one();
    ok()
}

/// Окно выбора папки в приложении хоста.
pub async fn pick_folder(State(app): State<Arc<App>>, m: Manager) -> Out {
    if !m.0.local || !app.host.can_pick() {
        return Err(ApiError::bad("no_picker", "Выбор папки - только в приложении на компьютере хоста"));
    }
    let a = app.clone();
    let path = tokio::task::spawn_blocking(move || a.host.pick_folder()).await.map_err(ApiError::internal)?;
    Ok(Json(json!({"path": path.map(|p| p.display().to_string())})))
}

pub async fn remove_library(State(app): State<Arc<App>>, _m: Manager, Path(id): Path<i64>) -> Out {
    app.db.tx(|tx| {
        tx.execute("DELETE FROM libraries WHERE id = ?", [id])?;
        tx.execute("DELETE FROM titles WHERE NOT EXISTS (SELECT 1 FROM files f WHERE f.title_id = titles.id)", [])
    })?;
    ok()
}

pub async fn rescan(State(app): State<Arc<App>>, _m: Manager) -> Out {
    app.scan_kick.notify_one();
    ok()
}

pub async fn refresh_all(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let n = app.db.with(|c| {
        c.execute("UPDATE titles SET meta_status = 'pending' WHERE locked = 0 AND meta_status != 'manual'", [])
    })?;
    app.meta_state.lock().error = None;
    app.meta_kick.notify_one();
    Ok(Json(json!({"ok": true, "count": n})))
}

#[derive(Deserialize)]
pub struct TitlesQuery {
    q: Option<String>,
    status: Option<String>,
}

pub async fn titles(State(app): State<Arc<App>>, _m: Manager, Query(q): Query<TitlesQuery>) -> Out {
    let rows: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT t.id, t.kind, t.title, t.year, t.poster, t.meta_status, t.hidden, t.locked, t.provider, t.provider_id,
                    (SELECT COUNT(*) FROM files f WHERE f.title_id = t.id AND f.missing = 0),
                    (SELECT f.path FROM files f WHERE f.title_id = t.id ORDER BY f.season, f.episode LIMIT 1)
             FROM titles t WHERE (?1 IS NULL OR t.meta_status = ?1 OR (?1 = 'hidden' AND t.hidden = 1))
             ORDER BY t.added_at DESC",
        )?;
        let r = st.query_map([q.status.as_deref().filter(|s| !s.is_empty())], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?, "kind": r.get::<_, String>(1)?, "title": r.get::<_, String>(2)?,
                "year": r.get::<_, Option<i32>>(3)?, "poster": img(r.get(4)?), "status": r.get::<_, String>(5)?,
                "hidden": r.get::<_, bool>(6)?, "locked": r.get::<_, bool>(7)?, "provider": r.get::<_, Option<String>>(8)?,
                "providerId": r.get::<_, Option<String>>(9)?, "files": r.get::<_, i64>(10)?, "path": r.get::<_, Option<String>>(11)?,
            }))
        })?;
        r.collect()
    })?;
    let rows = match q.q.as_deref().map(crate::util::norm).filter(|s| !s.is_empty()) {
        Some(needle) => rows
            .into_iter()
            .filter(|r| {
                crate::util::norm(&format!(
                    "{} {}",
                    r["title"].as_str().unwrap_or(""),
                    r["path"].as_str().unwrap_or("")
                ))
                .contains(&needle)
            })
            .collect(),
        None => rows,
    };
    Ok(Json(json!(rows)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleEdit {
    title: Option<String>,
    original_title: Option<String>,
    year: Option<Option<i32>>,
    overview: Option<String>,
    kind: Option<String>,
    hidden: Option<bool>,
    locked: Option<bool>,
    genres: Option<Vec<String>>,
    /// Возрастной рейтинг («12+», «PG-13»); пусто - неизвестен.
    age: Option<String>,
    /// Страны (на билете - первая).
    countries: Option<Vec<String>>,
    /// Режиссёр; пусто - убрать.
    director: Option<String>,
}

pub async fn edit_title(
    State(app): State<Arc<App>>,
    _m: Manager,
    Path(id): Path<i64>,
    Json(b): Json<TitleEdit>,
) -> Out {
    if let Some(k) = &b.kind {
        if k != "movie" && k != "series" {
            return Err(ApiError::bad("bad_value", "Вид - фильм или сериал"));
        }
    }
    if b.title.as_deref().is_some_and(|t| t.trim().is_empty()) {
        return Err(ApiError::bad("bad_title", "Название не может быть пустым"));
    }
    let manual = b.title.is_some()
        || b.overview.is_some()
        || b.genres.is_some()
        || b.year.is_some()
        || b.original_title.is_some()
        || b.countries.is_some()
        || b.director.is_some();
    let n = app.db.tx(|tx| {
        let mut n = 0;
        let mut set = |sql: &str, v: &dyn rusqlite::ToSql| -> rusqlite::Result<()> {
            n += tx.execute(
                &format!("UPDATE titles SET {sql} = ?, updated_at = ? WHERE id = ?"),
                params![v, now_ms(), id],
            )?;
            Ok(())
        };
        if let Some(v) = &b.title {
            set("title", &v.trim())?;
        }
        if let Some(v) = &b.original_title {
            set("original_title", &v.trim())?;
        }
        if let Some(v) = &b.year {
            set("year", v)?;
        }
        if let Some(v) = &b.overview {
            set("overview", &v.trim())?;
        }
        if let Some(v) = &b.kind {
            set("kind", v)?;
        }
        if let Some(v) = &b.hidden {
            set("hidden", v)?;
        }
        if let Some(v) = &b.locked {
            set("locked", v)?;
        }
        if let Some(v) = &b.age {
            let v = v.trim();
            set("age", &(!v.is_empty()).then_some(v))?;
            set("age_min", &crate::taste::age_min(v))?;
        }
        if let Some(v) = &b.genres {
            set("genres", &serde_json::to_string(&crate::meta::genres_ru(v)).unwrap_or_else(|_| "[]".into()))?;
        }
        if let Some(v) = &b.countries {
            let v: Vec<&str> = v.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
            set("countries", &serde_json::to_string(&v).unwrap_or_else(|_| "[]".into()))?;
        }
        if let Some(d) = &b.director {
            let people: String = tx
                .query_row("SELECT people FROM titles WHERE id = ?", [id], |r| r.get(0))
                .unwrap_or_else(|_| "[]".into());
            let mut people: Vec<crate::meta::Person> = serde_json::from_str(&people).unwrap_or_default();
            people.retain(|p| p.job != "director");
            if !d.trim().is_empty() {
                people.insert(
                    0,
                    crate::meta::Person {
                        name: d.trim().into(),
                        role: None,
                        job: "director".into(),
                        photo: None,
                        id: None,
                    },
                );
            }
            set("people", &serde_json::to_string(&people).unwrap_or_else(|_| "[]".into()))?;
        }
        if manual {
            // Хост поправил сам - автоматическое обновление это не перезапишет.
            set("locked", &true)?;
        }
        Ok(n)
    })?;
    if n == 0 {
        return Err(ApiError::not_found());
    }
    ok()
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
    year: Option<i32>,
    series: Option<bool>,
}

pub async fn search_meta(
    State(app): State<Arc<App>>,
    _m: Manager,
    Path(id): Path<i64>,
    Query(q): Query<SearchQuery>,
) -> Out {
    let (title, year, kind): (String, Option<i32>, String) = app.db.with(|c| {
        c.query_row("SELECT title, year, kind FROM titles WHERE id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
    })?;
    let p = Provider::from_settings(&app)
        .ok_or_else(|| ApiError::bad("no_provider", "Источник описаний выключен или нет ключа"))?;
    let query = q.q.filter(|s| !s.trim().is_empty()).unwrap_or(title);
    let res = p
        .search(&query, q.year.or(year), q.series.unwrap_or(kind == "series"))
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "provider", e.to_string()))?;
    Ok(Json(json!({"provider": p.name(), "results": res})))
}

#[derive(Deserialize)]
pub struct MatchBody {
    id: String,
}

pub async fn match_meta(
    State(app): State<Arc<App>>,
    _m: Manager,
    Path(id): Path<i64>,
    Json(b): Json<MatchBody>,
) -> Out {
    meta::match_manual(&app, id, &b.id)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "provider", e.to_string()))?;
    ok()
}

/// Убрать фильм из каталога - только без файлов на диске (импорт с сайтов, пустые карточки): фильм с
/// файлами вернулся бы при следующей проверке папки, его убирают вместе с файлами из папки.
pub async fn delete_title(State(app): State<Arc<App>>, _m: Manager, Path(id): Path<i64>) -> Out {
    let files: i64 =
        app.db.with(|c| c.query_row("SELECT COUNT(*) FROM files WHERE title_id = ?", [id], |r| r.get(0)))?;
    if files > 0 {
        return Err(ApiError::bad("has_files", "У фильма есть файлы - уберите их из папки, и он исчезнет сам"));
    }
    let n = app.db.with(|c| c.execute("DELETE FROM titles WHERE id = ?", [id]))?;
    if n == 0 {
        return Err(ApiError::not_found());
    }
    Ok(Json(json!({"ok": true})))
}

/// Убрать все фильмы без файлов разом.
pub async fn delete_empty_titles(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let n = app.db.with(|c| {
        c.execute("DELETE FROM titles WHERE NOT EXISTS (SELECT 1 FROM files f WHERE f.title_id = titles.id)", [])
    })?;
    Ok(Json(json!({"removed": n})))
}

pub async fn refresh_title(State(app): State<Arc<App>>, _m: Manager, Path(id): Path<i64>) -> Out {
    let pid: Option<(String, String)> = app.db.with(|c| {
        c.query_row("SELECT provider, provider_id FROM titles WHERE id = ? AND provider_id IS NOT NULL", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
    })?;
    let p = Provider::from_settings(&app)
        .ok_or_else(|| ApiError::bad("no_provider", "Источник описаний выключен или нет ключа"))?;
    let res = match pid {
        Some((prov, pid)) if prov == p.name() => meta::match_manual(&app, id, &pid).await.map(|_| true),
        _ => meta::fetch_one(&app, &p, id).await,
    };
    let found = res.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "provider", e.to_string()))?;
    Ok(Json(json!({"ok": true, "found": found})))
}

#[derive(Deserialize)]
pub struct MergeBody {
    into: i64,
}

pub async fn merge_title(
    State(app): State<Arc<App>>,
    _m: Manager,
    Path(id): Path<i64>,
    Json(b): Json<MergeBody>,
) -> Out {
    if id == b.into {
        return Err(ApiError::bad("same", "Нельзя объединить с самим собой"));
    }
    app.db.tx(|tx| {
        let exists: bool = tx.query_row("SELECT EXISTS (SELECT 1 FROM titles WHERE id = ?)", [b.into], |r| r.get(0))?;
        if !exists {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        tx.execute("UPDATE files SET title_id = ? WHERE title_id = ?", params![b.into, id])?;
        tx.execute("INSERT OR IGNORE INTO my_list (profile_id, title_id, added_at) SELECT profile_id, ?, added_at FROM my_list WHERE title_id = ?", params![b.into, id])?;
        tx.execute(
            "INSERT OR IGNORE INTO ratings (profile_id, title_id, value, updated_at) SELECT profile_id, ?, value, updated_at FROM ratings WHERE title_id = ?",
            params![b.into, id],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO watched (profile_id, title_id, at) SELECT profile_id, ?, at FROM watched WHERE title_id = ?",
            params![b.into, id],
        )?;
        tx.execute("DELETE FROM titles WHERE id = ?", [id])
    })?;
    ok()
}

pub async fn upload_image(
    State(app): State<Arc<App>>,
    _m: Manager,
    Path((id, which)): Path<(i64, String)>,
    body: Bytes,
) -> Out {
    let col = match which.as_str() {
        "poster" => "poster",
        "backdrop" => "backdrop",
        _ => return Err(ApiError::not_found()),
    };
    if body.len() > crate::images::MAX_IMAGE {
        return Err(ApiError::bad("too_big", "Картинка больше 15 МБ"));
    }
    let a = app.clone();
    let name = tokio::task::spawn_blocking(move || a.images.save(&body))
        .await
        .map_err(ApiError::internal)?
        .map_err(|e| ApiError::bad("bad_image", e.to_string()))?;
    app.db.with(|c| {
        c.execute(
            &format!("UPDATE titles SET {col} = ?, poster_sm = CASE WHEN '{col}' = 'poster' THEN NULL ELSE poster_sm END, backdrop_sm = CASE WHEN '{col}' = 'backdrop' THEN NULL ELSE backdrop_sm END, backdrop_md = CASE WHEN '{col}' = 'backdrop' THEN NULL ELSE backdrop_md END, locked = 1, updated_at = ? WHERE id = ?"),
            params![name, now_ms(), id],
        )
    })?;
    Ok(Json(json!({"ok": true, "url": format!("/img/{name}")})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEdit {
    season: Option<Option<i32>>,
    episode: Option<Option<i32>>,
    title_id: Option<i64>,
}

pub async fn edit_file(State(app): State<Arc<App>>, _m: Manager, Path(id): Path<i64>, Json(b): Json<FileEdit>) -> Out {
    app.db.tx(|tx| {
        if let Some(s) = b.season {
            tx.execute("UPDATE files SET season = ? WHERE id = ?", params![s, id])?;
        }
        if let Some(e) = b.episode {
            tx.execute("UPDATE files SET episode = ?, episode_end = NULL WHERE id = ?", params![e, id])?;
        }
        if let Some(t) = b.title_id {
            tx.execute(
                "UPDATE files SET title_id = ? WHERE id = ? AND EXISTS (SELECT 1 FROM titles WHERE id = ?)",
                params![t, id, t],
            )?;
            tx.execute("DELETE FROM titles WHERE NOT EXISTS (SELECT 1 FROM files f WHERE f.title_id = titles.id)", [])?;
        }
        Ok(())
    })?;
    ok()
}

// ---------- люди ----------

pub async fn users(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let rows: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT id, login, name, role, can_download, disabled, created_at, seen_at, pass IS NOT NULL FROM users
             ORDER BY role = 'host' DESC, role = 'admin' DESC, name COLLATE NOCASE",
        )?;
        let r = st.query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?, "login": r.get::<_, String>(1)?, "name": r.get::<_, String>(2)?,
                "role": r.get::<_, String>(3)?, "canDownload": r.get::<_, bool>(4)?, "disabled": r.get::<_, bool>(5)?,
                "createdAt": r.get::<_, i64>(6)?, "seenAt": r.get::<_, Option<i64>>(7)?, "hasPassword": r.get::<_, bool>(8)?,
            }))
        })?;
        r.collect()
    })?;
    Ok(Json(json!(rows)))
}

#[derive(Deserialize)]
pub struct NewUser {
    login: String,
    name: String,
    password: String,
    role: Option<String>,
}

fn role_allowed(by: &auth::Viewer, role: &str) -> ApiResult<()> {
    match role {
        "viewer" => Ok(()),
        "admin" if by.role == "host" => Ok(()),
        "admin" => {
            Err(ApiError::new(StatusCode::FORBIDDEN, "forbidden", "Назначать администраторов может только хост"))
        }
        _ => Err(ApiError::bad("bad_value", "Неизвестная роль")),
    }
}

pub async fn create_user(State(app): State<Arc<App>>, m: Manager, Json(b): Json<NewUser>) -> Out {
    let role = b.role.unwrap_or_else(|| "viewer".into());
    role_allowed(&m.0, &role)?;
    let login = auth::check_login(&b.login)?;
    let name = auth::check_name(&b.name)?;
    auth::check_password(&b.password)?;
    let pw = b.password;
    let hash = auth::password_job(move || auth::hash_password(&pw)).await.map_err(ApiError::internal)?;
    let n = app.db.with(|c| {
        c.execute(
            "INSERT OR IGNORE INTO users (login, name, pass, role, created_at) VALUES (?, ?, ?, ?, ?)",
            params![login, name, hash, role, now_ms()],
        )
    })?;
    if n == 0 {
        return Err(ApiError::new(StatusCode::CONFLICT, "login_taken", "Такой логин уже занят"));
    }
    ok()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserEdit {
    name: Option<String>,
    role: Option<String>,
    can_download: Option<bool>,
    disabled: Option<bool>,
    password: Option<String>,
}

fn target_role(app: &App, id: i64) -> ApiResult<String> {
    Ok(app.db.with(|c| c.query_row("SELECT role FROM users WHERE id = ?", [id], |r| r.get(0)))?)
}

pub async fn edit_user(State(app): State<Arc<App>>, m: Manager, Path(id): Path<i64>, Json(b): Json<UserEdit>) -> Out {
    let role = target_role(&app, id)?;
    if role == "host" && (b.role.is_some() || b.disabled == Some(true)) {
        return Err(ApiError::bad("host", "Хоста нельзя отключить или сменить ему роль"));
    }
    if role != "viewer" && m.0.role != "host" && m.0.id != id {
        return Err(ApiError::forbidden());
    }
    if let Some(r) = &b.role {
        role_allowed(&m.0, r)?;
    }
    if role == "host" && b.password.is_some() && !m.0.local {
        return Err(ApiError::forbidden());
    }
    let hash = match b.password {
        Some(p) => {
            auth::check_password(&p)?;
            Some(auth::password_job(move || auth::hash_password(&p)).await.map_err(ApiError::internal)?)
        }
        None => None,
    };
    let name = b.name.as_deref().map(auth::check_name).transpose()?;
    app.db.tx(|tx| {
        if let Some(n) = &name {
            tx.execute("UPDATE users SET name = ? WHERE id = ?", params![n, id])?;
            tx.execute("UPDATE profiles SET name = ? WHERE user_id = ?", params![n, id])?;
        }
        if let Some(r) = &b.role {
            tx.execute("UPDATE users SET role = ? WHERE id = ?", params![r, id])?;
        }
        if let Some(d) = b.can_download {
            tx.execute("UPDATE users SET can_download = ? WHERE id = ?", params![d, id])?;
        }
        if let Some(d) = b.disabled {
            tx.execute("UPDATE users SET disabled = ? WHERE id = ?", params![d, id])?;
            if d {
                tx.execute("DELETE FROM sessions WHERE user_id = ?", [id])?;
            }
        }
        if let Some(h) = &hash {
            tx.execute("UPDATE users SET pass = ? WHERE id = ?", params![h, id])?;
            tx.execute("DELETE FROM account_tokens WHERE user_id = ?", [id])?;
            tx.execute("DELETE FROM sessions WHERE user_id = ? AND local = 0", [id])?;
        }
        Ok(())
    })?;
    ok()
}

pub async fn delete_user(State(app): State<Arc<App>>, m: Manager, Path(id): Path<i64>) -> Out {
    let role = target_role(&app, id)?;
    if role == "host" || (role == "admin" && m.0.role != "host") || id == m.0.id {
        return Err(ApiError::forbidden());
    }
    app.db.with(|c| c.execute("DELETE FROM users WHERE id = ?", [id]))?;
    ok()
}

// ---------- приглашения ----------

fn invite_link(app: &App, code: &str) -> Option<String> {
    app.access.public_url(app).map(|u| format!("{u}/join/{code}"))
}

type InviteRow = (String, Option<String>, Option<i64>, Option<i64>, i64, i64);

pub async fn invites(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let rows: Vec<InviteRow> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT code, note, uses_left, expires_at, used, created_at FROM invites ORDER BY created_at DESC",
        )?;
        let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
        r.collect()
    })?;
    let now = now_ms();
    let out: Vec<Value> = rows
        .into_iter()
        .map(|(code, note, uses, exp, used, created)| {
            json!({
                "link": invite_link(&app, &code), "code": code, "note": note, "usesLeft": uses, "expiresAt": exp,
                "used": used, "createdAt": created,
                "active": uses.is_none_or(|u| u > 0) && exp.is_none_or(|e| e > now),
            })
        })
        .collect();
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
pub struct NewInvite {
    note: Option<String>,
    uses: Option<i64>,
    days: Option<i64>,
}

pub async fn create_invite(State(app): State<Arc<App>>, _m: Manager, Json(b): Json<NewInvite>) -> Out {
    let code = human_code(8);
    let exp = b.days.filter(|d| *d > 0).map(|d| now_ms() + d.min(365) * 86_400_000);
    let uses = b.uses.filter(|u| *u > 0);
    let note = b.note.map(|n| n.trim().chars().take(80).collect::<String>()).filter(|n| !n.is_empty());
    app.db.with(|c| {
        c.execute(
            "INSERT INTO invites (code, note, uses_left, expires_at, created_at) VALUES (?, ?, ?, ?, ?)",
            params![code, note, uses, exp, now_ms()],
        )
    })?;
    Ok(Json(json!({"code": code, "link": invite_link(&app, &code)})))
}

pub async fn delete_invite(State(app): State<Arc<App>>, _m: Manager, Path(code): Path<String>) -> Out {
    app.db.with(|c| c.execute("DELETE FROM invites WHERE code = ?", [code]))?;
    ok()
}

// ---------- доступ из интернета ----------

pub async fn access(State(app): State<Arc<App>>, _m: Manager) -> Out {
    Ok(Json(json!(app.access.view(&app))))
}

#[derive(Deserialize)]
pub struct AccessBody {
    mode: String,
    subdomain: Option<String>,
    url: Option<String>,
}

pub async fn set_access(State(app): State<Arc<App>>, _m: Manager, Json(b): Json<AccessBody>) -> Out {
    match b.mode.as_str() {
        "lan" | "cloudpub" => {}
        "fxtunnel" => {
            if let Some(s) = b.subdomain {
                let s = s.trim().to_lowercase();
                if !SUBDOMAIN.is_match(&s) {
                    return Err(ApiError::bad("bad_subdomain", "Адрес: 3-32 латинские буквы, цифры и «-»"));
                }
                app.db.set_setting("access.fx.sub", &s)?;
            } else if app.db.setting_or("access.fx.sub", "").is_empty() {
                app.db.set_setting("access.fx.sub", &crate::access::suggest_subdomain())?;
            }
        }
        "custom" => {
            let u = url_or_empty(b.url.as_deref().unwrap_or(""))?;
            if u.is_empty() {
                return Err(ApiError::bad("bad_url", "Укажите адрес, по которому сайт открывается из интернета"));
            }
            app.db.set_setting("access.url", &u)?;
        }
        _ => return Err(ApiError::bad("bad_value", "Неизвестный режим доступа")),
    }
    app.db.set_setting("access.mode", &b.mode)?;
    Access::restart(&app);
    Ok(Json(json!(app.access.view(&app))))
}

pub async fn access_login(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let l = Access::start_login(&app).await.map_err(|e| {
        ApiError::new(StatusCode::BAD_GATEWAY, "fxtunnel", format!("Не удалось связаться с fxTunnel: {e}"))
    })?;
    Ok(Json(json!(l)))
}

#[derive(Deserialize)]
pub struct CloudPubLogin {
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    token: String,
}

pub async fn cloudpub_login(State(app): State<Arc<App>>, _m: Manager, Json(b): Json<CloudPubLogin>) -> Out {
    let (email, token) = (b.email.trim(), b.token.trim());
    if token.is_empty() && (email.is_empty() || b.password.is_empty()) {
        return Err(ApiError::bad("login", "Введите почту и пароль от CloudPub или ключ API"));
    }
    if !token.is_empty() && (token.len() < 16 || token.chars().any(char::is_whitespace)) {
        return Err(ApiError::bad("token", "Вставьте ключ API из личного кабинета CloudPub целиком"));
    }
    Access::cp_login(&app, email, &b.password, token)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "cloudpub", e.to_string()))?;
    Ok(Json(json!(app.access.view(&app))))
}

pub async fn cloudpub_logout(State(app): State<Arc<App>>, _m: Manager) -> Out {
    Access::cp_logout(&app).await;
    Ok(Json(json!(app.access.view(&app))))
}

pub async fn access_logout(State(app): State<Arc<App>>, _m: Manager) -> Out {
    Access::logout(&app);
    ok()
}

/// Журнал входов по паролю: кто, удачно ли, с какого браузера (адреса не храним).
pub async fn logins(State(app): State<Arc<App>>, _m: Manager) -> Out {
    let items: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT l.login, u.name, l.ok, l.agent, l.at FROM logins l LEFT JOIN users u ON u.id = l.user_id
             ORDER BY l.id DESC LIMIT 100",
        )?;
        let r = st.query_map([], |r| {
            Ok(json!({"login": r.get::<_, String>(0)?, "name": r.get::<_, Option<String>>(1)?, "ok": r.get::<_, bool>(2)?,
                      "agent": r.get::<_, Option<String>>(3)?, "at": r.get::<_, i64>(4)?}))
        })?;
        r.collect()
    })?;
    Ok(Json(json!({"items": items})))
}
