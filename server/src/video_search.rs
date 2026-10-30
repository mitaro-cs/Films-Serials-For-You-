//! Совпадение произведения и серии важнее качества и позиции в выдаче провайдера.
use crate::{
    api::links::{parse_rutube, parse_vk, SearchQuery},
    util, App,
};
use parking_lot::Mutex;
use reqwest::Url;
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock, Weak},
    time::Duration,
};
type SearchResult = Result<Vec<Value>, String>;
type Cache = Mutex<HashMap<String, (i64, SearchResult)>>;
fn ttl(result: &SearchResult) -> i64 {
    if result.is_ok() {
        600_000
    } else {
        5_000
    }
}
fn cached(key: &str) -> Option<anyhow::Result<Vec<Value>>> {
    cache()
        .lock()
        .get(key)
        .filter(|(at, result)| util::now_ms() - at < ttl(result))
        .map(|(_, result)| result.clone().map_err(anyhow::Error::msg))
}
fn remember(key: String, result: SearchResult) {
    let mut c = cache().lock();
    let now = util::now_ms();
    c.retain(|_, (at, result)| now - *at < ttl(result));
    if c.len() >= 128 && !c.contains_key(&key) {
        if let Some(oldest) = c.iter().min_by_key(|(_, (at, _))| *at).map(|(key, _)| key.clone()) {
            c.remove(&oldest);
        }
    }
    c.insert(key, (now, result));
}
fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}
fn search_lock(key: &str) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS.get_or_init(Mutex::default).lock();
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    locks.insert(key.to_owned(), Arc::downgrade(&lock));
    lock
}
fn years() -> &'static regex::Regex {
    static YEARS: OnceLock<regex::Regex> = OnceLock::new();
    YEARS.get_or_init(|| regex::Regex::new(r"\b(?:19|20)\d{2}\b").expect("static years"))
}
async fn get(app: &App, url: Url) -> Option<Value> {
    app.http.get(url).timeout(Duration::from_secs(5)).send().await.ok()?.error_for_status().ok()?.json().await.ok()
}
async fn archive(app: &App, title: &str) -> Option<Vec<Value>> {
    let safe = title.replace(['"', '\\'], " ");
    let query = format!("mediatype:movies AND title:\"{safe}\" AND licenseurl:(*publicdomain*)");
    let u = Url::parse_with_params(
        "https://archive.org/advancedsearch.php",
        &[
            ("q", query.as_str()),
            ("output", "json"),
            ("rows", "8"),
            ("fl[]", "identifier"),
            ("fl[]", "title"),
            ("fl[]", "licenseurl"),
        ],
    )
    .ok()?;
    let results = get(app, u).await?;
    let rows = results["response"]["docs"]
        .as_array()?
        .iter()
        .filter(|v| v["licenseurl"].as_str().is_some_and(|s| s.contains("creativecommons.org/publicdomain/")))
        .take(3);
    let items = futures_util::future::join_all(rows.map(|row| async move {
        let id = row["identifier"].as_str().filter(|s| s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_- .".contains(&b)))?;
        let meta = get(app, Url::parse(&format!("https://archive.org/metadata/{id}")).ok()?).await?;
        let file = meta["files"].as_array()?.iter()
            .filter(|f| f["name"].as_str().is_some_and(|s| s.ends_with(".mp4")))
            .max_by_key(|f| f["format"] == "h.264 IA")?;
        let name = file["name"].as_str()?;
        let mut url = Url::parse("https://archive.org/download/").ok()?;
        url.path_segments_mut().ok()?.pop_if_empty().push(id).push(name);
        Some(json!({"site":"archive","url":url.as_str(),"label":row["title"],"thumbUrl":format!("https://archive.org/services/img/{id}"),"duration":file["length"].as_str().and_then(|s|s.parse::<f64>().ok()).map(|s|s.round() as i64),"author":"Internet Archive · Public Domain","license":row["licenseurl"]}))
    })).await;
    let out = items.into_iter().flatten().collect();
    Some(out)
}
/// Служебные слова в подписях трейлеров - не часть названия фильма.
const TRAILER_WORDS: &[&str] = &[
    "трейлер",
    "трейлеры",
    "trailer",
    "тизер",
    "teaser",
    "русский",
    "русском",
    "русская",
    "на",
    "дублированный",
    "дублированная",
    "дубляж",
    "озвучка",
    "озвучкой",
    "в",
    "с",
    "и",
    "официальный",
    "official",
    "финальный",
    "final",
    "новый",
    "второй",
    "hd",
    "full",
    "fullhd",
    "uhd",
    "4k",
    "4к",
    "1080p",
    "720p",
    "2160p",
    "1080",
    "720",
    "фильм",
    "фильма",
    "сериал",
    "сериала",
    "сезон",
    "season",
    "кино",
    "кинотеатрах",
    "премьера",
    "скоро",
    "the",
    "of",
    "and",
    "a",
    "movie",
    "film",
    "series",
    "rus",
    "ru",
    "dub",
    "subs",
];

/// Трейлер именно этого фильма: в подписи кроме названия (или оригинального названия) - только
/// служебные слова и числа. «Начало конца - трейлер» для «Начала» и «Дюна: Часть вторая» для «Дюны»
/// не подходят - это другие фильмы, хоть все слова названия в подписи есть.
pub fn trailer_exact(label: &str, titles: &[&str]) -> bool {
    let words = |s: &str| util::norm(s).split_whitespace().map(String::from).collect::<Vec<_>>();
    let allowed: std::collections::HashSet<String> = titles.iter().flat_map(|t| words(t)).collect();
    words(label)
        .iter()
        .all(|w| allowed.contains(w) || TRAILER_WORDS.contains(&w.as_str()) || w.chars().all(|c| c.is_ascii_digit()))
}

/// В подписи явно низкое качество (360p, 480p) - для трейлера не годится.
pub fn low_quality(label: &str) -> bool {
    let l = label.to_lowercase();
    ["240p", "360p", "480p", " 360", " 480"].iter().any(|t| l.contains(t))
}

pub fn matches(
    label: &str,
    title: &str,
    season: Option<i64>,
    episode: Option<i64>,
    year: Option<i64>,
    trailer: bool,
) -> bool {
    let text = util::norm(label);
    let name = util::norm(title);
    let words = name
        .split_whitespace()
        .filter(|w| !matches!(*w, "в" | "на" | "и" | "с" | "the" | "of" | "and" | "a" | "фильм" | "сериал"))
        .collect::<Vec<_>>();
    if words.is_empty() || !words.iter().all(|w| text.split_whitespace().any(|token| token == *w)) {
        return false;
    }
    let preview =
        text.contains("трейлер") || text.contains("trailer") || text.contains("тизер") || text.contains("teaser");
    if trailer {
        if let Some(year) = year {
            let years = years();
            let found = years.find_iter(label).filter_map(|m| m.as_str().parse::<i64>().ok()).collect::<Vec<_>>();
            if !found.is_empty() && !found.iter().any(|v| (*v - year).abs() <= 1) {
                return false;
            }
        }
        return preview;
    }
    if preview || text.contains("обзор") || text.contains("рецензия") {
        return false;
    }
    if let (Some(s), Some(e)) = (season, episode) {
        let pattern = format!(
            r"(?:^|\D)(?:s0*{s}\s*e0*{e}|0*{s}\s*сезон\s*0*{e}\s*серия|сезон\s*0*{s}\s*серия\s*0*{e}|season\s*0*{s}\s*episode\s*0*{e})(?:\D|$)"
        );
        if !regex::Regex::new(&pattern).expect("numeric episode pattern").is_match(&text) {
            return false;
        }
    } else if let Some(y) = year {
        let years = years();
        let found = years.find_iter(label).filter_map(|m| m.as_str().parse::<i64>().ok()).collect::<Vec<_>>();
        if !found.is_empty() && !found.contains(&y) {
            return false;
        }
        // Одно общее слово не отличает фильм от ролика про животных, игры и т.п.
        if words.len() == 1 && !found.contains(&y) {
            return false;
        }
    }
    true
}
pub async fn find(app: &Arc<App>, q: &SearchQuery) -> anyhow::Result<Vec<Value>> {
    let query = q.q.trim();
    let provider = q.provider.as_deref();
    anyhow::ensure!(
        provider.is_none_or(|p| matches!(p, "rutube" | "vk" | "archive" | "peertube" | "commons" | "lordfilm")),
        "Неизвестный источник"
    );
    let enabled = |name: &str| provider.is_none_or(|p| p == name);
    if query.chars().count() < 2 {
        return Ok(vec![]);
    }
    anyhow::ensure!(query.chars().count() <= 180, "Поисковый запрос слишком длинный");
    let token = app.db.setting_or("vk.token", "");
    let key = format!(
        "{}:{}:{}:{}:{}:{}:{}",
        util::norm(query),
        q.min.unwrap_or(0),
        q.title_id.unwrap_or(0),
        q.ext_id.as_deref().unwrap_or(""),
        q.season.unwrap_or(0),
        q.episode.unwrap_or(0),
        q.mode.as_deref().unwrap_or("")
    );
    let key = format!("{key}:{}:{}", util::sha256_hex(token.as_bytes()), provider.unwrap_or("all"));
    if let Some(result) = cached(&key) {
        return result;
    }
    // Waiters recheck the cache after the first search; cancellation releases the lock.
    let lock = search_lock(&key);
    let _search = lock.lock().await;
    if let Some(result) = cached(&key) {
        return result;
    }
    static SEARCH_SLOTS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    let slot = tokio::time::timeout(
        Duration::from_secs(10),
        SEARCH_SLOTS.get_or_init(|| tokio::sync::Semaphore::new(4)).acquire(),
    )
    .await;
    let _slot = match slot {
        Ok(permit) => permit?,
        Err(_) => {
            let message = "Поиск занят. Повторите через несколько секунд";
            remember(key, Err(message.to_owned()));
            anyhow::bail!(message);
        }
    };
    let expected: Option<(String, Option<String>, Option<i64>)> = if let Some(id) = q.title_id {
        app.db.with(|c| {
            c.query_row("SELECT title,original_title,year FROM titles WHERE id=?", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()
        })?
    } else if let Some(id) = &q.ext_id {
        app.db.with(|c|c.query_row("SELECT COALESCE((SELECT title FROM title_localizations WHERE id=e.id),title),original,year FROM ext_titles e WHERE id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional())?
    } else {
        None
    };
    let ru = async {
        if !enabled("rutube") {
            return None;
        }
        get(app, Url::parse_with_params("https://rutube.ru/api/search/video/", &[("query", query)]).ok()?)
            .await
            .map(|v| parse_rutube(&v))
    };
    let vk = async {
        if !enabled("vk") || token.is_empty() {
            return None;
        }
        get(
            app,
            Url::parse_with_params(
                "https://api.vk.com/method/video.search",
                &[("q", query), ("count", "30"), ("adult", "0"), ("access_token", token.as_str()), ("v", "5.199")],
            )
            .ok()?,
        )
        .await
        .map(|v| parse_vk(&v))
    };
    // Dailymotion проверен отдельно: metadata доступна, но CDN может запретить нативный поток.
    // Не выдаём такой результат за рабочую альтернативу; явная ссылка поддерживается отдельно.
    let archive_title = expected
        .as_ref()
        .and_then(|(_, original, _)| original.as_deref())
        .or_else(|| expected.as_ref().map(|(name, _, _)| name.as_str()))
        .unwrap_or(query);
    let open = async {
        if !enabled("archive") || q.mode.as_deref() == Some("trailer") || q.season.is_some() {
            return None;
        }
        tokio::time::timeout(Duration::from_secs(5), archive(app, archive_title)).await.ok().flatten()
    };
    let peer = async {
        if !enabled("peertube") {
            return None;
        }
        tokio::time::timeout(Duration::from_secs(6), crate::video_sources::peertube(app, query, q.min.unwrap_or(0)))
            .await
            .ok()
            .flatten()
    };
    let wiki = async {
        if !enabled("commons") || q.season.is_some() || q.mode.as_deref() == Some("trailer") {
            return None;
        }
        tokio::time::timeout(Duration::from_secs(5), crate::video_sources::commons(app, archive_title))
            .await
            .ok()
            .flatten()
    };
    let lord = async {
        if !enabled("lordfilm") {
            return None;
        }
        if q.mode.as_deref() == Some("trailer") || q.season.is_some() != q.episode.is_some() {
            return Some(vec![]);
        }
        let (name, year) = expected.as_ref().map(|(name, _, year)| (name.as_str(), *year)).unwrap_or((query, None));
        if let (Some(season), Some(episode)) = (q.season, q.episode) {
            return tokio::time::timeout(
                Duration::from_secs(15),
                crate::lordfilm::search_episode(name, year, season, episode),
            )
            .await
            .ok()
            .flatten();
        }
        tokio::time::timeout(Duration::from_secs(10), crate::lordfilm::search(name, year)).await.ok().flatten()
    };
    let (ru, vk, open, peer, wiki, lord) = tokio::join!(ru, vk, open, peer, wiki, lord);
    if ru.is_none() && vk.is_none() && open.is_none() && peer.is_none() && wiki.is_none() && lord.is_none() {
        let message = "Источники видео сейчас не отвечают";
        remember(key, Err(message.to_owned()));
        anyhow::bail!(message);
    }
    let mut out = ru
        .into_iter()
        .chain(vk)
        .chain(open)
        .chain(peer)
        .chain(wiki)
        .chain(lord)
        .flatten()
        // LordFilm movie cards do not publish a duration. Do not invent one or drop exact movie matches.
        .filter(|i| i["site"] == "lordfilm" || q.min.is_none_or(|m| i["duration"].as_i64().unwrap_or(0) >= m))
        .filter(|i| {
            let label = i["label"].as_str().unwrap_or("");
            if let Some((title, original, year)) = &expected {
                matches(label, title, q.season, q.episode, *year, q.mode.as_deref() == Some("trailer"))
                    || original.as_ref().filter(|name| !name.trim().is_empty()).is_some_and(|name| {
                        matches(label, name, q.season, q.episode, *year, q.mode.as_deref() == Some("trailer"))
                    })
            } else {
                q.mode.as_deref() != Some("trailer")
            }
        })
        .collect::<Vec<_>>();
    if q.mode.as_deref() == Some("trailer") {
        out.retain(|v| v["duration"].as_i64().is_some_and(|d| (20..=360).contains(&d)));
        if let Some((title, original, _)) = &expected {
            let names: Vec<&str> = std::iter::once(title.as_str()).chain(original.as_deref()).collect();
            out.retain(|v| {
                let label = v["label"].as_str().unwrap_or("");
                trailer_exact(label, &names) && !low_quality(label)
            });
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|i| seen.insert(i["url"].as_str().unwrap_or("").to_string()));
    out.sort_by_key(|i| {
        (
            std::cmp::Reverse(crate::api::links::quality_hint(i["label"].as_str().unwrap_or(""))),
            if i["site"] == "archive" {
                0
            } else if i["site"] == "vk" {
                1
            } else {
                2
            },
        )
    });
    out.truncate(36);
    for i in &mut out {
        i["thumb"] = json!(i["thumbUrl"].as_str().map(|u| {
            let mut p = Url::parse("http://localhost/api/links/thumbnail").expect("static URL");
            p.query_pairs_mut().append_pair("url", u);
            format!("/api/links/thumbnail?{}", p.query().unwrap_or_default())
        }));
    }
    remember(key, Ok(out.clone()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn fifty_waiters_share_failure_and_retry_after_cooldown() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..50 {
            let calls = calls.clone();
            tasks.push(tokio::spawn(async move {
                let key = "test-failed-provider";
                let lock = search_lock(key);
                let _guard = lock.lock().await;
                if cached(key).is_none() {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    remember(key.to_owned(), Err("offline".to_owned()));
                }
                assert!(cached(key).unwrap().is_err());
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        cache().lock().get_mut("test-failed-provider").unwrap().0 -= 5_001;
        assert!(cached("test-failed-provider").is_none());
        remember("test-failed-provider".to_owned(), Ok(vec![json!({"url":"recovered"})]));
        assert_eq!(cached("test-failed-provider").unwrap().unwrap()[0]["url"], "recovered");
    }
    #[tokio::test]
    async fn identical_searches_share_lock_and_cancel_safely() {
        let first = search_lock("same-film");
        let second = search_lock("same-film");
        assert!(Arc::ptr_eq(&first, &second));
        let guard = first.lock().await;
        assert!(second.try_lock().is_err());
        assert!(search_lock("other-film").try_lock().is_ok());
        drop(guard);
        assert!(second.try_lock().is_ok());
        let weak = Arc::downgrade(&first);
        drop(first);
        drop(second);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn correct_episode_and_movie() {
        assert!(!matches("Принц Персии — русский трейлер", "", None, None, None, true));
        assert!(!matches("Смешные домашние животные 2025", "Животные", None, None, Some(2026), false));
        assert!(!matches("Невероятно смешные животные", "Животные", None, None, Some(2026), false));
        assert!(matches("Животные | Animals (2026)", "Животные", None, None, Some(2026), false));
        assert!(!matches(
            "Секс в большом городе (фильм, 2008)",
            "Секс в большом городе",
            Some(1),
            Some(1),
            Some(1998),
            false
        ));
        assert!(!matches(
            "Теория Большого взрыва 1 сезон 1 серия",
            "Секс в большом городе",
            Some(1),
            Some(1),
            None,
            false
        ));
        assert!(matches(
            "Секс в большом городе 1 сезон 1 серия",
            "Секс в большом городе",
            Some(1),
            Some(1),
            None,
            false
        ));
        assert!(matches("Sex and the City S01E01", "Sex and the City", Some(1), Some(1), None, false));
        assert!(!matches("Sex and the City S01E10", "Sex and the City", Some(1), Some(1), None, false));
        assert!(!matches(
            "Секс в большом городе 1 сезон 10 серия",
            "Секс в большом городе",
            Some(1),
            Some(1),
            None,
            false
        ));
        assert!(!matches("Матрица 2021 трейлер", "Матрица", None, None, Some(1999), true));
        assert!(!matches("Матрица: Трейлер", "Матрица", None, None, Some(1999), false));
        assert!(!matches("Матрица 2021", "Матрица", None, None, Some(1999), false));
    }
}

#[cfg(test)]
mod trailer_tests {
    use super::*;

    #[test]
    fn only_this_films_trailer() {
        let t = ["Начало", "Inception"];
        assert!(trailer_exact("Начало (2010) - русский дублированный трейлер 1080p", &t));
        assert!(trailer_exact("Inception - Official Trailer HD", &t));
        assert!(!trailer_exact("Начало конца - трейлер 2024", &t));
        let d = ["Дюна", "Dune"];
        assert!(!trailer_exact("Дюна: Часть вторая - русский трейлер", &d));
        assert!(trailer_exact("Дюна — финальный русский трейлер (2021)", &d));
        assert!(!trailer_exact("Дюна обзор фильма трейлер", &d));
    }

    #[test]
    fn low_quality_labels() {
        assert!(low_quality("Трейлер 480p"));
        assert!(!low_quality("Трейлер 1080p"));
        assert!(!low_quality("Трейлер"));
    }
}
