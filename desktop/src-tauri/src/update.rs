//! Обновление с GitHub: `latest.json` последнего выпуска (подпись проверяет плагин по ключу из
//! tauri.conf.json). Проверка через 20 с после запуска и раз в 6 часов; нашлась новая версия —
//! пункт в трее и один раз за запуск вопрос «Обновить сейчас?». Перед установкой сайт
//! выключается как положено (база закрыта), потом приложение перезапускается уже новым.

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
}

impl Updates {
    pub fn set_item(&self, item: MenuItem<Wry>) {
        *self.item.lock() = Some(item);
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
            check(&app, false).await;
            tokio::time::sleep(EVERY).await;
        }
    });
}

/// `manual` — из меню трея: сообщить и «нет обновлений», и ошибку.
pub async fn check(app: &AppHandle, manual: bool) {
    let u = app.state::<Updates>();
    if *u.busy.lock() {
        return;
    }
    u.label("Проверяем обновления…", false);
    let found = match app.updater() {
        Ok(up) => up.check().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(Some(update)) => {
            let version = update.version.clone();
            u.label(&format!("Обновить до {version}"), true);
            *u.found.lock() = Some(update);
            let first = !std::mem::replace(&mut *u.asked.lock(), true);
            if first || manual {
                ask(app, &version);
            }
        }
        Ok(None) => {
            u.label("Проверить обновления", true);
            if manual {
                info(app, &format!("Установлена последняя версия — {}.", app.package_info().version));
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "проверка обновлений");
            u.label("Проверить обновления", true);
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
            check(&h, true).await
        }
    });
}

async fn install(app: &AppHandle) {
    let u = app.state::<Updates>();
    let Some(update) = u.found.lock().take() else { return };
    *u.busy.lock() = true;
    u.label("Скачиваем обновление…", false);
    let bytes = match update.download(|_, _| {}, || {}).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "скачивание обновления");
            *u.busy.lock() = false;
            u.label(&format!("Обновить до {}", update.version), true);
            *u.found.lock() = Some(update);
            info(app, "Не удалось скачать обновление — проверьте интернет и попробуйте ещё раз из меню.");
            return;
        }
    };
    u.label("Устанавливаем…", false);
    // Сайт — выключить аккуратно до замены файлов приложения.
    crate::stop_server(app).await;
    if let Err(e) = update.install(bytes) {
        tracing::error!(error = %e, "установка обновления");
        info(app, "Не удалось установить обновление. Приложение перезапустится в прежней версии.");
    }
    app.restart();
}
