//! Обновление с GitHub: `latest.json` последнего выпуска (подпись проверяет плагин по ключу из
//! tauri.conf.json). Проверка через 20 с после запуска и раз в 6 часов; нашлась новая версия -
//! пункт в трее и один раз за запуск вопрос «Обновить сейчас?». Перед установкой сайт
//! выключается как положено (база закрыта), потом приложение перезапускается уже новым.
//! То же из окна: «Управление → Версия» (состояние - `info`, без системных окон).
//!
//! Ставим только подписанное нашим ключом (секрет выпуска `TAURI_SIGNING_PRIVATE_KEY`, открытый
//! ключ - в tauri.conf.json): неподписанный установщик не скачиваем даже с GitHub. Перед установкой -
//! копия базы в `backups/`: новая версия может обновить базу, и старая её уже не откроет.

use fsfy::host::UpdateInfo;
use parking_lot::Mutex;
use std::time::Duration;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST: Duration = Duration::from_secs(20);
const EVERY: Duration = Duration::from_secs(6 * 3600);

#[derive(Default)]
pub struct Updates {
    found: Mutex<Option<Update>>,
    item: Mutex<Option<MenuItem<Wry>>>,
    asked: Mutex<bool>,
    busy: Mutex<bool>,
    info: Mutex<UpdateInfo>,
}

/// Кто спросил: проверка по расписанию (один раз - окно «Обновить?»), трей (окна всегда) или
/// страница сайта (без окон: ответ - в `info`).
#[derive(Clone, Copy, PartialEq)]
pub enum From {
    Auto,
    Menu,
    Page,
}

impl Updates {
    pub fn set_item(&self, item: MenuItem<Wry>) {
        *self.item.lock() = Some(item);
    }

    pub fn info(&self) -> UpdateInfo {
        let mut i = self.info.lock().clone();
        if i.state.is_empty() {
            i.state = "idle".into();
        }
        i
    }

    fn set(&self, f: impl FnOnce(&mut UpdateInfo)) {
        f(&mut self.info.lock());
    }

    fn label(&self, text: &str, enabled: bool) {
        if let Some(i) = self.item.lock().as_ref() {
            let _ = i.set_text(text);
            let _ = i.set_enabled(enabled);
        }
    }
}

/// Фоновая проверка на всё время работы приложения.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST).await;
        loop {
            check(&app, From::Auto).await;
            tokio::time::sleep(EVERY).await;
        }
    });
}

/// Из трея - сообщить и «нет обновлений», и ошибку; со страницы - только в `info`.
pub async fn check(app: &AppHandle, from: From) {
    let u = app.state::<Updates>();
    if *u.busy.lock() {
        return;
    }
    u.label("Проверяем обновления…", false);
    u.set(|i| {
        i.state = "checking".into();
        i.error = None;
    });
    let found = match app.updater() {
        Ok(up) => up.check().await,
        Err(e) => Err(e),
    };
    let current = app.package_info().version.to_string();
    let version = match found {
        Ok(Some(update)) => {
            let v = update.version.clone();
            *u.found.lock() = Some(update);
            Ok(Some(v))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    };
    match version {
        Ok(Some(version)) => {
            u.label(&format!("Обновить до {version}"), true);
            u.set(|i| {
                i.state = "available".into();
                i.latest = Some(version.clone());
                i.error = None;
                i.checked_at = Some(fsfy::util::now_ms());
            });
            let first = !std::mem::replace(&mut *u.asked.lock(), true);
            if from != From::Page && (first || manual_ask(from)) {
                ask(app, &version);
            }
        }
        Ok(None) => {
            u.label("Проверить обновления", true);
            u.set(|i| {
                i.state = "latest".into();
                i.latest = None;
                i.error = None;
                i.checked_at = Some(fsfy::util::now_ms());
            });
            if from == From::Menu {
                info(app, &format!("Установлена последняя версия - {current}."));
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "проверка обновлений");
            u.label("Проверить обновления", true);
            u.set(|i| {
                i.state = "error".into();
                i.error = Some(
                    "Не удалось проверить - нет связи с GitHub или новый выпуск ещё собирается. Попробуйте позже."
                        .into(),
                );
                i.checked_at = Some(fsfy::util::now_ms());
            });
            if from == From::Menu {
                info(app, "Не удалось проверить обновления - нет связи с GitHub. Попробуем позже сами.");
            }
        }
    }
}

fn manual_ask(from: From) -> bool {
    from == From::Menu
}

fn info(app: &AppHandle, text: &str) {
    app.dialog().message(text).title(crate::TITLE).kind(MessageDialogKind::Info).show(|_| {});
}

fn ask(app: &AppHandle, version: &str) {
    let h = app.clone();
    app.dialog()
        .message(format!(
            "Доступна новая версия {version}. Обновить сейчас?\n\nСайт выключится примерно на минуту и включится сам."
        ))
        .title(crate::TITLE)
        .buttons(MessageDialogButtons::OkCancelCustom("Обновить".into(), "Позже".into()))
        .show(move |yes| {
            if yes {
                tauri::async_runtime::spawn(async move { install(&h).await });
            }
        });
}

/// Из трея: «Обновить до …» - ставим найденное; иначе - проверить вручную.
pub fn menu(app: &AppHandle) {
    let h = app.clone();
    let u = app.state::<Updates>();
    let has = u.found.lock().is_some();
    tauri::async_runtime::spawn(async move {
        if has {
            install(&h).await
        } else {
            check(&h, From::Menu).await
        }
    });
}

/// Со страницы сайта: найдено - ставим, нет - сначала проверяем.
pub fn from_page(app: &AppHandle, install_now: bool) {
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        let ready = h.state::<Updates>().found.lock().is_some();
        if !install_now || !ready {
            check(&h, From::Page).await;
        }
        if install_now {
            install(&h).await;
        }
    });
}

pub async fn install(app: &AppHandle) {
    let u = app.state::<Updates>();
    let Some(update) = u.found.lock().take() else { return };
    *u.busy.lock() = true;
    u.label("Скачиваем обновление…", false);
    u.set(|i| {
        i.state = "downloading".into();
        i.progress = Some(0.0);
    });
    let done = std::sync::atomic::AtomicUsize::new(0);
    let h = app.clone();
    let bytes = match update
        .download(
            move |chunk, total| {
                let got = done.fetch_add(chunk, std::sync::atomic::Ordering::Relaxed) + chunk;
                if let Some(t) = total.filter(|t| *t > 0) {
                    h.state::<Updates>().set(|i| i.progress = Some((got as f32 / t as f32).min(1.0)));
                }
            },
            || {},
        )
        .await
    {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "скачивание обновления");
            *u.busy.lock() = false;
            u.label(&format!("Обновить до {}", update.version), true);
            *u.found.lock() = Some(update);
            u.set(|i| {
                i.state = "error".into();
                i.progress = None;
                i.error = Some("Не удалось скачать обновление - проверьте интернет и попробуйте ещё раз.".into());
            });
            return;
        }
    };
    u.label("Устанавливаем…", false);
    u.set(|i| {
        i.state = "installing".into();
        i.progress = Some(1.0);
    });
    // Сайт - выключить аккуратно до замены файлов приложения.
    crate::stop_server(app).await;
    // Без копии базы не обновляемся: откатиться на старую версию без неё нельзя.
    if let Err(e) = backup_db(app) {
        tracing::error!(error = %e, "копия базы перед обновлением");
        info(app, "Не удалось сохранить копию базы - обновление отменено. Приложение перезапустится.");
        app.restart();
    }
    if let Err(e) = update.install(bytes) {
        tracing::error!(error = %e, "установка обновления");
        info(app, "Не удалось установить обновление. Приложение перезапустится в прежней версии.");
    }
    app.restart();
}

/// Сколько копий базы хранить: старые удаляются.
const BACKUPS: usize = 3;

/// Копия `fsfy.db` (сайт уже выключен, база закрыта) в `backups/fsfy-<время>-<версия>.db`.
fn backup_db(app: &AppHandle) -> std::io::Result<()> {
    let data = app.path().app_data_dir().map_err(std::io::Error::other)?;
    let db = data.join("fsfy.db");
    if !db.exists() {
        return Ok(());
    }
    let dir = data.join("backups");
    std::fs::create_dir_all(&dir)?;
    // Время первым: имена сортируются по времени (у версий «0.10» < «0.9» как у строк).
    let name = format!("fsfy-{}-{}.db", fsfy::util::now_ms(), app.package_info().version);
    std::fs::copy(&db, dir.join(&name))?;
    // Обычно журнал при закрытии уже влит в базу; если нет - копируем и его.
    let wal = data.join("fsfy.db-wal");
    if wal.metadata().is_ok_and(|m| m.len() > 0) {
        std::fs::copy(&wal, dir.join(format!("{name}-wal")))?;
    }
    let mut old: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "db"))
        .collect();
    old.sort();
    for p in old.iter().rev().skip(BACKUPS) {
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(p.with_extension("db-wal"));
    }
    Ok(())
}
