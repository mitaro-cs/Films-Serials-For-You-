//! Обновление с GitHub: `latest.json` последнего выпуска (подпись проверяет плагин по ключу из
//! tauri.conf.json). Проверка через 20 с после запуска и раз в 6 часов; нашлась новая версия -
//! пункт в трее и один раз за запуск вопрос «Обновить сейчас?». Перед установкой сайт
//! выключается как положено (база закрыта), потом приложение перезапускается уже новым.
//! То же из окна: «Управление → Версия» (состояние - `info`, без системных окон).
//!
//! Нет `latest.json` (выпуск собран без секрета подписи) - запасная дорога через GitHub API
//! (`fsfy::release`): скачиваем установщик своей системы и запускаем его (Windows - сам ставит
//! поверх, Mac - открывается .dmg).

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
    manual: Mutex<Option<fsfy::release::Latest>>,
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
    // Без файлов автообновления в выпуске - смотрим сам выпуск на GitHub.
    let manual = match &found {
        Ok(Some(_)) => None,
        _ => Some(fsfy::release::latest(&fsfy::release::client()).await),
    };
    let current = app.package_info().version.to_string();
    let version = match (found, manual) {
        (Ok(Some(update)), _) => {
            let v = update.version.clone();
            *u.found.lock() = Some(update);
            Ok(Some(v))
        }
        (_, Some(Ok(l))) if fsfy::release::newer(&l.version, &current) && l.asset.is_some() => {
            let v = l.version.clone();
            *u.manual.lock() = Some(l);
            Ok(Some(v))
        }
        (_, Some(Ok(_))) | (Ok(None), _) => Ok(None),
        (Err(e), Some(Err(e2))) => Err(format!("{e}; {e2}")),
        (Err(e), None) => Err(e.to_string()),
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
                i.error = Some("Не удалось проверить - нет связи с GitHub. Попробуйте позже.".into());
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
    let has = u.found.lock().is_some() || u.manual.lock().is_some();
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
        let ready = {
            let u = h.state::<Updates>();
            u.found.lock().is_some() || u.manual.lock().is_some()
        };
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
    let Some(update) = u.found.lock().take() else {
        let manual = u.manual.lock().take();
        if let Some(l) = manual {
            install_manual(app, l).await;
        }
        return;
    };
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
    if let Err(e) = update.install(bytes) {
        tracing::error!(error = %e, "установка обновления");
        info(app, "Не удалось установить обновление. Приложение перезапустится в прежней версии.");
    }
    app.restart();
}

/// Запасная дорога: скачать установщик с GitHub и запустить. Windows ставит поверх себя сам
/// (тихий режим с окном хода `/P`), на Mac открывается .dmg - перетащить в «Программы».
async fn install_manual(app: &AppHandle, l: fsfy::release::Latest) {
    let u = app.state::<Updates>();
    let Some((name, url)) = l.asset.clone() else { return };
    *u.busy.lock() = true;
    u.label("Скачиваем обновление…", false);
    u.set(|i| {
        i.state = "downloading".into();
        i.progress = Some(0.0);
        i.error = None;
    });
    let to = std::env::temp_dir().join(&name);
    let h = app.clone();
    let got = fsfy::release::download(&fsfy::release::client(), &url, &to, l.sha256.as_deref(), move |done, total| {
        if let Some(t) = total.filter(|t| *t > 0) {
            h.state::<Updates>().set(|i| i.progress = Some((done as f32 / t as f32).min(1.0)));
        }
    })
    .await;
    *u.busy.lock() = false;
    if let Err(e) = got {
        tracing::warn!(error = %e, "скачивание установщика");
        u.label(&format!("Обновить до {}", l.version), true);
        u.set(|i| {
            i.state = "error".into();
            i.progress = None;
            i.error = Some("Не удалось скачать обновление - проверьте интернет и попробуйте ещё раз.".into());
        });
        *u.manual.lock() = Some(l);
        return;
    }
    u.set(|i| {
        i.state = "installing".into();
        i.progress = Some(1.0);
    });
    if cfg!(target_os = "windows") {
        u.label("Устанавливаем…", false);
        crate::stop_server(app).await;
        match std::process::Command::new(&to).arg("/P").spawn() {
            Ok(_) => app.exit(0),
            Err(e) => {
                tracing::error!(error = %e, "запуск установщика");
                info(app, "Не удалось запустить установщик. Откройте его вручную из папки загрузок.");
            }
        }
    } else {
        let _ = std::process::Command::new("open").arg(&to).spawn();
        u.label(&format!("Обновить до {}", l.version), true);
        info(
            app,
            &format!(
                "Версия {} скачана. Перетащите приложение в «Программы» с заменой и откройте его снова.",
                l.version
            ),
        );
    }
}
