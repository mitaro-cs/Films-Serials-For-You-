//! Связь с оболочкой приложения (Tauri): выбор папки, адрес для трея.
//! Без оболочки (`fsfy serve`) — заглушка: папки вводятся путём, события пишутся в лог.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum HostEvent {
    /// Адрес сайта для группы изменился (None — доступа из интернета нет).
    Access { url: Option<String>, lan: Option<String> },
}

/// Обновление приложения хоста с GitHub — для «Управление → Версия».
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// idle — не проверяли, checking, latest — последняя, available, downloading, installing, error.
    pub state: String,
    /// Найденная новая версия.
    pub latest: Option<String>,
    /// Скачано, 0..1.
    pub progress: Option<f32>,
    pub error: Option<String>,
    pub checked_at: Option<i64>,
}

pub trait Host: Send + Sync + 'static {
    /// Обновление из приложения; None — сервер запущен сам по себе (`fsfy serve`), обновлять нечего.
    fn update_info(&self) -> Option<UpdateInfo> {
        None
    }
    /// Проверить сейчас (без окон — ответ появится в `update_info`).
    fn update_check(&self) {}
    /// Скачать и поставить найденное: сайт выключится и приложение перезапустится.
    fn update_install(&self) {}
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
