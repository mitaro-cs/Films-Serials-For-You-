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
    /// Где создать свою папку для фильмов («Фильмы»/«Видео» пользователя). None — не создавать.
    fn media_dir(&self) -> Option<PathBuf> {
        None
    }
    /// Показать папку в Finder/Проводнике.
    fn reveal(&self, _path: &std::path::Path) -> bool {
        false
    }
    fn event(&self, e: HostEvent) {
        tracing::info!(?e, "событие");
    }
}

pub struct Headless;
impl Host for Headless {}
