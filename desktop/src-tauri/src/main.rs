//! Приложение хоста: сервер работает внутри (без отдельных процессов и Java), окно открывает
//! сайт на 127.0.0.1, значок в трее держит сайт включённым, когда окно закрыто.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fsfy::host::{Host, HostEvent};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

const TITLE: &str = "Films & Serials For You";

#[derive(Default)]
struct DesktopHost {
    app: OnceLock<AppHandle>,
    url: Mutex<Option<String>>,
    link_item: Mutex<Option<MenuItem<Wry>>>,
}

impl Host for DesktopHost {
    fn can_pick(&self) -> bool {
        true
    }

    fn pick_folder(&self) -> Option<PathBuf> {
        let app = self.app.get()?;
        app.dialog().file().set_title("Папка с фильмами и сериалами").blocking_pick_folder()?.into_path().ok()
    }

    fn event(&self, e: HostEvent) {
        match e {
            HostEvent::Access { url, lan } => {
                let label = match (&url, &lan) {
                    (Some(_), _) => "Скопировать ссылку для друзей",
                    (None, Some(_)) => "Скопировать ссылку (домашняя сеть)",
                    _ => "Ссылки пока нет",
                };
                *self.url.lock() = url.or(lan);
                if let Some(item) = self.link_item.lock().as_ref() {
                    let _ = item.set_text(label);
                    let _ = item.set_enabled(self.url.lock().is_some());
                }
            }
        }
    }
}

struct State {
    running: Mutex<Option<fsfy::Running>>,
    _host: Arc<DesktopHost>,
    _awake: Mutex<Option<keepawake::KeepAwake>>,
}

fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .setup(|app| {
            let handle = app.handle().clone();
            let host = Arc::new(DesktopHost::default());
            let _ = host.app.set(handle.clone());

            let data = app.path().app_data_dir()?;
            let opts = fsfy::Options { data, port: None, window_port: None };
            let running = match tauri::async_runtime::block_on(fsfy::start(opts, host.clone())) {
                Ok(r) => r,
                Err(e) => {
                    let h = handle.clone();
                    handle
                        .dialog()
                        .message(format!("Не удалось запустить сервер: {e:#}"))
                        .title(TITLE)
                        .kind(MessageDialogKind::Error)
                        .show(move |_| h.exit(1));
                    return Ok(());
                }
            };
            let entry: tauri::Url = running.entry_url().parse()?;

            let hidden = std::env::args().any(|a| a == "--hidden");
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(entry))
                .title(TITLE)
                .inner_size(1360.0, 860.0)
                .min_inner_size(960.0, 620.0)
                .background_color(tauri::window::Color(11, 10, 26, 255))
                .visible(!hidden)
                .build()?;

            // Трей: сайт работает, пока работает приложение, даже с закрытым окном.
            let open = MenuItem::with_id(app, "open", "Открыть", true, None::<&str>)?;
            let link = MenuItem::with_id(app, "link", "Ссылки пока нет", false, None::<&str>)?;
            let browser = MenuItem::with_id(app, "browser", "Открыть в браузере", true, None::<&str>)?;
            let autostart = CheckMenuItem::with_id(
                app,
                "autostart",
                "Запускать при входе в систему",
                true,
                app.autolaunch().is_enabled().unwrap_or(false),
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Выключить сайт и выйти", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&open, &link, &browser, &sep, &autostart, &sep2, &quit])?;
            *host.link_item.lock() = Some(link);
            // Адрес мог прийти раньше, чем появилось меню.
            let v = running.app.access.view(&running.app);
            host.event(HostEvent::Access { url: v.public_url, lan: v.lan_url });

            let tray_host = host.clone();
            let port = running.app.ports.0;
            let auto = autostart.clone();
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().ok_or("нет значка")?)
                .tooltip(TITLE)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, e| match e.id().as_ref() {
                    "open" => show(app),
                    "link" => {
                        if let Some(u) = tray_host.url.lock().clone() {
                            let _ = app.clipboard().write_text(u);
                        }
                    }
                    "browser" => {
                        let u = tray_host.url.lock().clone().unwrap_or_else(|| format!("http://localhost:{port}"));
                        let _ = app.opener().open_url(u, None::<&str>);
                    }
                    "autostart" => {
                        let al = app.autolaunch();
                        let on = al.is_enabled().unwrap_or(false);
                        let _ = if on { al.disable() } else { al.enable() };
                        let _ = auto.set_checked(al.is_enabled().unwrap_or(false));
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Компьютер не засыпает, пока работает сайт (экран гаснуть может).
            let awake = keepawake::Builder::default()
                .idle(true)
                .reason("Раздаём фильмы и сериалы")
                .app_name(TITLE)
                .app_reverse_domain("app.fsfy")
                .create()
                .map_err(|e| tracing::warn!(error = %e, "не удалось запретить сон"))
                .ok();

            app.manage(State { running: Mutex::new(Some(running)), _host: host, _awake: Mutex::new(awake) });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Закрыли окно — сайт продолжает работать в трее.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("приложение не собралось");

    app.run(|app, event| match event {
        RunEvent::Exit => {
            if let Some(state) = app.try_state::<State>() {
                if let Some(r) = state.running.lock().take() {
                    tauri::async_runtime::block_on(r.shutdown());
                }
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show(app),
        _ => {}
    });
}
