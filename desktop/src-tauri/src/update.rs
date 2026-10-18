//! Обновление с GitHub: `latest.json` последнего выпуска (подпись проверяет плагин по ключу из
//! tauri.conf.json). Проверка через 20 с после запуска и раз в 6 часов; нашлась новая версия —
//! пункт в трее и один раз за запуск вопрос «Обновить сейчас?». Перед установкой сайт
//! выключается как положено (база закрыта), потом приложение перезапускается уже новым.
//! То же из окна: «Управление → Версия» (состояние — `info`, без системных окон).

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

/// Кто спросил: проверка по расписанию (один раз — окно «Обновить?»), трей (окна всегда) или
/// страница сайта (без окон: ответ — в `info`).
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

/// Из трея — сообщить и «нет обновлений», и ошибку; со страницы — только в `info`.
pub async fn check(app: &AppHandle, from: From) {
    let u = app.state::<Updates>();
    if *u.busy.lock() {
        return;
    }
    let manual = from == From::Menu;
    u.label("Проверяем обновления…", false);
    u.set(|i| {
        i.state = "checking".into();
        i.error = None;
    });
    let found = match app.updater() {
        Ok(up) => up.check().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(Some(update)) => {
            let version = update.version.clone();
            u.label(&format!("Обновить до {version}"), true);
            u.set(|i| {
                i.state = "available".into();
                i.latest = Some(version.clone());
                i.checked_at = Some(fsfy::util::now_ms());
            });
            *u.found.lock() = Some(update);
            let first = !std::mem::replace(&mut *u.asked.lock(), true);
            if from != From::Page && (first || manual) {
                ask(app, &version);
            }
        }
        Ok(None) => {
            u.label("Проверить обновления", true);
            u.set(|i| {
                i.state = "latest".into();
                i.latest = None;
                i.checked_at = Some(fsfy::util::now_ms());
            });
            if manual {
                info(app, &format!("Установлена последняя версия — {}.", app.package_info().version));
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "проверка обновлений");
            u.label("Проверить обновления", true);
            u.set(|i| {
                i.state = "error".into();
                i.error = Some("Не удалось проверить — нет связи с GitHub. Попробуйте позже.".into());
                i.checked_at = Some(fsfy::util::now_ms());
            });
            if manual {
                info(app, "Не удалось проверить обновления — нет связи с GitHub. Попробуем позже сами.");
            }
        }
    }
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

/// Из трея: «Обновить до …» — ставим найденное; иначе — проверить вручную.
pub fn menu(app: &AppHandle) {
    let h = app.clone();
    let has = app.state::<Updates>().found.lock().is_some();
    tauri::async_runtime::spawn(async move {
        if has {
            install(&h).await
        } else {
            check(&h, From::Menu).await
        }
    });
}

/// Со страницы сайта: найдено — ставим, нет — сначала проверяем.
pub fn from_page(app: &AppHandle, install_now: bool) {
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        if !install_now || h.state::<Updates>().found.lock().is_none() {
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
                i.error = Some("Не удалось скачать обновление — проверьте интернет и попробуйте ещё раз.".into());
            });
            return;
        }
    };
    u.label("Устанавливаем…", false);
    u.set(|i| {
        i.state = "installing".into();
        i.progress = Some(1.0);
    });
    // Сайт — выключить аккуратно до замены файлов приложения.
    crate::stop_server(app).await;
    if let Err(e) = update.install(bytes) {
        tracing::error!(error = %e, "установка обновления");
        info(app, "Не удалось установить обновление. Приложение перезапустится в прежней версии.");
    }
    app.restart();
}
