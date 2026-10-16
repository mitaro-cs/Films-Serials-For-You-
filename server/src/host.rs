//! Связь с оболочкой приложения (Tauri): выбор папки, адрес для трея.
//! Без оболочки (`fsfy serve`) — заглушка: папки вводятся путём, события пишутся в лог.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum HostEvent {
    /// Адрес сайта для группы изменился (None — доступа из интернета нет).
    Access { url: Option<String>, lan: Option<String> },
}

pub trait Host: Send + Sync + 'static {
    /// Окно выбора папки (блокирующий вызов, зовётся не из async-кода).
    fn pick_folder(&self) -> Option<PathBuf> {
        None
    }
    fn can_pick(&self) -> bool {
        false
    }
    fn event(&self, e: HostEvent) {
        tracing::info!(?e, "событие");
    }
}

pub struct Headless;
impl Host for Headless {}
