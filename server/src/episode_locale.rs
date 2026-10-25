//! Локализация серий отдельно от данных провайдера: обновление IMDb/TVMaze
//! не стирает перевод, а старый свежий сезон тоже получает русские названия.
use crate::{
    ruwiki::has_cyrillic,
    util::{now_ms, sha256_hex},
    App,
};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

pub fn key(text: &str) -> String {
    format!("episode.ru:{}", sha256_hex(text.trim().as_bytes()))
}
pub fn localized(app: &App, text: Option<&str>) -> Option<String> {
    let text = text?.trim();
    if text.is_empty() {
        return None;
    }
    if app.db.setting_or("meta.language", "ru") == "source"
        || has_cyrillic(text)
        || !text.chars().any(char::is_alphabetic)
    {
        return Some(text.to_owned());
    }
    app.db.setting(&key(text)).filter(|s| has_cyrillic(s))
}

/// Запрос UI не ждёт сеть. Неудачу повторяем не чаще раза в сутки.
pub fn queue(app: &Arc<App>, texts: Vec<String>) {
    static LIMIT: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    if app.db.setting_or("meta.language", "ru") == "source" {
        return;
    }
    for text in texts
        .into_iter()
        .filter(|text| !text.trim().is_empty() && text.len() <= 450 && localized(app, Some(text)).is_none())
        .take(80)
    {
        let cache = key(&text);
        let attempt = format!("{cache}.at");
        let last = app.db.setting_or(&attempt, "0").parse::<i64>().unwrap_or(0);
        if now_ms() - last < 86_400_000 || !app.detail_jobs.lock().insert(cache.clone()) {
            continue;
        }
        let a = app.clone();
        let limit = LIMIT.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2))).clone();
        tokio::spawn(async move {
            let Ok(_permit) = limit.acquire_owned().await else {
                a.detail_jobs.lock().remove(&cache);
                return;
            };
            let _ = a.db.set_setting(&attempt, &now_ms().to_string());
            if let Some(translated) = translate(&a.http, &text).await {
                let _ = a.db.set_setting(&cache, &translated);
            }
            a.detail_jobs.lock().remove(&cache);
        });
    }
}

async fn translate(http: &reqwest::Client, text: &str) -> Option<String> {
    // MyMemory accepts at most 500 UTF-8 bytes per query. Long synopses are
    // left unavailable rather than silently truncating their meaning.
    if text.len() > 450 {
        return None;
    }
    let url = reqwest::Url::parse_with_params(
        "https://api.mymemory.translated.net/get",
        &[("q", text), ("langpair", "en|ru")],
    )
    .ok()?;
    let value: serde_json::Value =
        http.get(url).timeout(Duration::from_secs(8)).send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    if value["responseStatus"].as_i64() != Some(200) {
        return None;
    }
    let translated = value["responseData"]["translatedText"].as_str()?.trim();
    (has_cyrillic(translated) && !translated.to_uppercase().contains("MYMEMORY"))
        .then(|| translated.replace("&#39;", "'").replace("&quot;", "\"").replace("&amp;", "&"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_is_bound_to_original_text() {
        assert_eq!(key(" Plan B "), key("Plan B"));
        assert_ne!(key("Plan B"), key("Plan C"));
    }
}
