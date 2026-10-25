//! Bounded, host-only import of a catalogue page. No background whole-site crawl.
use crate::{
    auth::Viewer,
    error::{ApiError, ApiResult},
    outbound, util, App,
};
use axum::{extract::State, Json};
use futures_util::{stream, StreamExt};
use reqwest::Url;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

#[derive(Deserialize)]
pub struct Input {
    url: String,
}

async fn page(url: &Url) -> anyhow::Result<String> {
    anyhow::ensure!(outbound::valid_url(url), "Нужен публичный адрес сайта");
    tokio::time::timeout(Duration::from_secs(10), super::stream_link::page(url.as_str())).await?
}

pub(crate) fn catalogue(html: &str, base: &Url) -> Vec<Url> {
    let links = regex::Regex::new(r#"(?i)<a\b[^>]*href\s*=\s*["']([^"']+)["']"#).unwrap();
    let film_path = regex::Regex::new(r"/\d+[-_][^/]+").unwrap();
    let mut seen = std::collections::HashSet::new();
    links
        .captures_iter(html)
        .filter_map(|c| base.join(&c[1].replace("&amp;", "&")).ok())
        .filter(|u| u.host_str() == base.host_str() && film_path.is_match(u.path()) && u.query().is_none())
        .filter(|u| seen.insert(u.to_string()))
        .take(20)
        .collect()
}

pub(crate) fn movie(html: &str) -> Option<(String, i64)> {
    // A series page can have a bare title/year H1 just like a movie.
    let series = regex::Regex::new(r#"(?is)(?:schema\.org/TVSeries|"@type"\s*:\s*"TVSeries"|/embed/(?:series|serial)/|<h[23]\b[^>]*>\s*(?:Про що|О чем|О чём)\s+(?:Серіал|Сериал))"#).unwrap();
    if series.is_match(html) {
        return None;
    }
    let h1 = regex::Regex::new(r"(?is)<h1\b[^>]*>(.*?)</h1>").unwrap().captures(html)?.get(1)?.as_str();
    let text = regex::Regex::new(r"<[^>]+>").unwrap().replace_all(h1, " ");
    let lower = text.to_lowercase();
    if lower.contains("сериал") || lower.contains("сезон") {
        return None;
    }
    let re = regex::Regex::new(r"^\s*(.+?)\s*\(((?:19|20)\d{2})\)").unwrap();
    let caps = re.captures(&text)?;
    let title = caps[1].trim().replace("&amp;", "&").replace("&quot;", "\"");
    if title.is_empty() || title.chars().count() > 200 {
        return None;
    }
    Some((title, caps[2].parse().ok()?))
}

pub async fn import(State(app): State<Arc<App>>, v: Viewer, Json(input): Json<Input>) -> ApiResult<Json<Value>> {
    if !v.local {
        return Err(ApiError::not_found());
    }
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = LOCK.try_lock().map_err(|_| ApiError::bad("busy", "Импорт уже выполняется"))?;
    let base =
        Url::parse(input.url.trim()).map_err(|_| ApiError::bad("url", "Укажите полный адрес страницы каталога"))?;
    if !outbound::valid_url(&base) {
        return Err(ApiError::bad("url", "Нужен публичный адрес сайта"));
    }
    let (base, html) = tokio::time::timeout(Duration::from_secs(10), super::stream_link::page_with_url(base.as_str()))
        .await
        .map_err(|_| ApiError::bad("site", "Сайт не ответил вовремя"))?
        .map_err(|e| ApiError::bad("site", format!("Не удалось открыть каталог: {e}")))?;
    let mut urls = catalogue(&html, &base);
    if movie(&html).is_some() {
        urls = vec![base.clone()];
    } else if regex::Regex::new(r"/\d+[-_][^/]+").unwrap().is_match(base.path()) {
        // Do not import recommendations from a rejected series/detail page.
        return Err(ApiError::bad(
            "title",
            "Эта карточка не распознана как фильм. Сериалы пока подключаются по отдельным сериям.",
        ));
    }
    if urls.is_empty() {
        // A site's collections/home page often links to the actual movie catalogue.
        let links = regex::Regex::new(r#"(?i)<a\b[^>]*href\s*=\s*["']([^"']+)["']"#).unwrap();
        let section = links.captures_iter(&html).filter_map(|c| base.join(&c[1]).ok()).find(|u| {
            u.host_str() == base.host_str()
                && matches!(u.path(), "/filmy/" | "/films/" | "/movies/")
                && u.query().is_none()
        });
        if let Some(section) = section {
            if let Ok(html) = page(&section).await {
                urls = catalogue(&html, &section);
            }
        }
    }
    if urls.is_empty() {
        return Err(ApiError::bad(
            "catalogue",
            "На этой странице не найдены карточки фильмов. Откройте раздел с фильмами и вставьте его адрес.",
        ));
    }
    let mut results = stream::iter(urls.into_iter().map(|url| async move {
        let result = page(&url).await;
        (url, result)
    }))
    .buffer_unordered(2);
    let mut added = 0;
    let mut existing = 0;
    let mut skipped = Vec::new();
    while let Some((url, result)) = results.next().await {
        let Ok(html) = result else {
            skipped.push(json!({"url":url.as_str(),"reason":"Страница недоступна"}));
            continue;
        };
        let Some((name, year)) = movie(&html) else {
            skipped.push(json!({"url":url.as_str(),"reason":"Не удалось подтвердить название и год фильма"}));
            continue;
        };
        if super::stream_link::find_iframes(&html, &url).is_empty() && super::stream_link::find_media(&html).is_none() {
            skipped.push(json!({"url":url.as_str(),"reason":"Не найден встроенный плеер"}));
            continue;
        }
        let key = format!("site:{}:{year}", util::norm(&name));
        let inserted = app.db.with(|c| {
            let tx = c.unchecked_transaction()?;
            let old: Option<i64> = tx
                .query_row(
                    "SELECT title_id FROM links WHERE url=? AND title_id IS NOT NULL LIMIT 1",
                    [url.as_str()],
                    |r| r.get(0),
                )
                .optional()?;
            if old.is_some() {
                return Ok(false);
            }
            tx.execute(
                "INSERT OR IGNORE INTO titles(kind,key,title,year,added_at,updated_at) VALUES('movie',?,?,?,?,?)",
                params![key, name, year, util::now_ms(), util::now_ms()],
            )?;
            let id: i64 = tx.query_row("SELECT id FROM titles WHERE key=?", [&key], |r| r.get(0))?;
            tx.execute(
                "INSERT INTO links(title_id,kind,url,label,added_by,added_at) VALUES(?,'site',?,?,?,?)",
                params![id, url.as_str(), name, v.profile.id, util::now_ms()],
            )?;
            tx.commit()?;
            Ok(true)
        })?;
        if inserted {
            added += 1;
        } else {
            existing += 1;
        }
    }
    app.meta_kick.notify_one();
    Ok(Json(
        json!({"added":added,"existing":existing,"skipped":skipped,"note":"Импортированы ссылки на фильмы с этой страницы (до 20). Доступность видео проверяется при открытии; для следующей порции вставьте адрес следующей страницы каталога."}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_catalogue_and_identity() {
        let base = Url::parse("https://example.org/home/").unwrap();
        let links = catalogue(
            r#"<a href='/383_Film/'>film</a><a href='/383_Film/'>dup</a><a href='https://other.org/1_film/'>ad</a><a href='/2026/'>year</a>"#,
            &base,
        );
        assert_eq!(links.len(), 1);
        assert_eq!(movie("<h1>Гарри Поттер (2001) Гідонлайн</h1>"), Some(("Гарри Поттер".into(), 2001)));
        assert!(movie("<h1>Сериал (2026)</h1>").is_none());
        assert!(movie("<h1>Смотреть фильмы онлайн</h1>").is_none());
        assert!(movie("<h1>Яростная (2026) Гідонлайн</h1><h2>Про що Серіал «Яростная» :</h2>").is_none());
    }
}
