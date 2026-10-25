//! Explicit local-only night action; never changes the Windows power plan.
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

static BUSY: AtomicBool = AtomicBool::new(false);

pub fn action(app: &tauri::AppHandle, action: &str, lights: bool) -> Value {
    if !cfg!(target_os = "windows") {
        return json!({"ok":false,"error":"Ночной режим доступен в Windows"});
    }
    if BUSY.swap(true, Ordering::AcqRel) {
        return json!({"ok":false,"error":"Дождитесь завершения предыдущей команды"});
    }
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::Release);
        }
    }
    let _release = Release;
    if action == "off" && !app.try_state::<crate::State>().is_some_and(|s| s._awake.lock().is_some()) {
        return json!({"ok":false,"error":"Не удалось защитить хост от сна. Проверьте параметры питания Windows."});
    }
    let rgb = if lights || action != "off" { lighting(app, action) } else { json!({"ok":true,"changed":0}) };
    if action == "off" {
        // Let the pointer settle and the HTTP response reach the local window first.
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
            #[cfg(target_os = "windows")]
            unsafe {
                // Documented WM_SYSCOMMAND / SC_MONITORPOWER; finite timeout for hung apps.
                #[link(name = "user32")]
                unsafe extern "system" {
                    fn SendMessageTimeoutW(
                        hwnd: *mut std::ffi::c_void,
                        msg: u32,
                        wparam: usize,
                        lparam: isize,
                        flags: u32,
                        timeout: u32,
                        result: *mut usize,
                    ) -> isize;
                }
                let mut result = 0;
                SendMessageTimeoutW(0xffffusize as *mut _, 0x112, 0xf170, 2, 2, 1000, &mut result);
            }
        });
        json!({"ok":true,"displayScheduled":true,"delay":3,"lights":rgb})
    } else {
        rgb
    }
}

fn lighting(app: &tauri::AppHandle, action: &str) -> Value {
    let result = (|| -> Result<Value, String> {
        let root = app.path().resource_dir().map_err(|e| e.to_string())?.join("resources/night");
        let data = app.path().app_data_dir().map_err(|e| e.to_string())?.join("night-light.json");
        let mut command = std::process::Command::new(root.join("NovaNightLight.exe"));
        command
            .arg(action)
            .arg(data)
            .current_dir(&root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|_| "Компонент управления подсветкой не найден".to_string())?;
        let start = std::time::Instant::now();
        loop {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(20) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    "Mystic Light не ответил. Настройки подсветки сохранены, если изменение уже началось.".into()
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        serde_json::from_slice(&output.stdout).map_err(|_| "Не удалось получить ответ Mystic Light".into())
    })();
    result.unwrap_or_else(|error| json!({"ok":false,"error":error}))
}
