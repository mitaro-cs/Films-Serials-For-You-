//! Описание на русском из русской Википедии: IMDb даёт сюжет только по-английски. По id IMDb
//! (свойство P345 в Викиданных) находим статью, берём начало раздела «Сюжет» (нет его - начало
//! статьи). Короткий слоган переводим MyMemory (бесплатно, без ключа). Не нашлось - остаётся как было.

use crate::App;
use reqwest::Url;
use rusqlite::params;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

const UA: &str = concat!("Nova/", env!("CARGO_PKG_VERSION"), " (home cinema)");
/// Описание - до стольких знаков, по границе предложения.
const MAX: usize = 700;

pub fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|c| matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё'))
}

async fn json(http: &reqwest::Client, url: Url) -> Option<Value> {
    http.get(url).header("user-agent", UA).timeout(Duration::from_secs(10)).send().await.ok()?.json().await.ok()
}

/// Обрезать по последней точке до `MAX` знаков.
pub fn trim(text: &str) -> String {
    let t = text.trim().replace(['-', '-'], "-");
    let t = t.as_str();
    if t.chars().count() <= MAX {
        return t.to_string();
    }
    let cut: String = t.chars().take(MAX).collect();
    match cut.rfind(['.', '!', '?']) {
        Some(i) if i > MAX / 3 => cut[..=i].to_string(),
        _ => format!("{}…", cut.trim_end()),
    }
}

/// Текст раздела «Сюжет» из полной статьи (без подзаголовков сезонов).
pub fn plot_section(full: &str) -> Option<String> {
    let start = full.find("== Сюжет ==")?;
    let rest = &full[start + "== Сюжет ==".len()..];
    let mut out = Vec::new();
    for line in rest.lines() {
        let l = line.trim();
        if l.starts_with("== ") {
            break;
        }
        if l.is_empty() || l.starts_with("===") {
            continue;
        }
        out.push(l);
        if out.iter().map(|s| s.len()).sum::<usize>() > MAX * 3 {
            break;
        }
    }
    (!out.is_empty()).then(|| trim(&out.join(" ")))
}

/// Описание фильма или сериала на русском по id IMDb.
pub async fn overview(http: &reqwest::Client, imdb: &str) -> Option<String> {
    let search = Url::parse_with_params(
        "https://www.wikidata.org/w/api.php",
        &[
            ("action", "query"),
            ("list", "search"),
            ("srsearch", &format!("haswbstatement:P345={imdb}")),
            ("format", "json"),
        ],
    )
    .ok()?;
    let q = json(http, search).await?["query"]["search"][0]["title"].as_str()?.to_string();
    let ent =
        json(http, Url::parse(&format!("https://www.wikidata.org/wiki/Special:EntityData/{q}.json")).ok()?).await?;
    let title = ent["entities"][&q]["sitelinks"]["ruwiki"]["title"].as_str()?.to_string();
    let text = Url::parse_with_params(
        "https://ru.wikipedia.org/w/api.php",
        &[("action", "query"), ("prop", "extracts"), ("explaintext", "1"), ("titles", &title), ("format", "json")],
    )
    .ok()?;
    let pages = json(http, text).await?;
    let full = pages["query"]["pages"].as_object()?.values().next()?["extract"].as_str()?.to_string();
    if let Some(p) = plot_section(&full) {
        return Some(p);
    }
    // Нет сюжета - начало статьи (до первого раздела).
    let intro = full.split("\n== ").next().unwrap_or("").replace('\n', " ");
    (!intro.trim().is_empty()).then(|| trim(&intro))
}

/// Короткая фраза (слоган) по-русски - MyMemory, до 200 знаков.
pub async fn translate(http: &reqwest::Client, text: &str) -> Option<String> {
    if text.chars().count() > 200 || has_cyrillic(text) {
        return None;
    }
    let u = Url::parse_with_params("https://api.mymemory.translated.net/get", &[("q", text), ("langpair", "en|ru")])
        .ok()?;
    let v = json(http, u).await?;
    let t = v["responseData"]["translatedText"].as_str()?.trim().to_string();
    (has_cyrillic(&t) && !t.to_uppercase().contains("MYMEMORY")).then_some(t)
}

/// Описания по-английски - по одному в фоне на русский (свои и из ленты). Не нашлось - помечаем,
/// чтобы не спрашивать по кругу (`settings ruwiki.tried` - до 5000 id).
pub async fn backfill(app: Arc<App>) {
    tokio::time::sleep(Duration::from_secs(30)).await;
    loop {
        let tried: Vec<String> = crate::api::json_list(app.db.setting("ruwiki.tried"));
        let todo: Vec<(bool, String, Option<String>)> = app
            .db
            .with(|c| {
                let mut st = c.prepare(
                    "SELECT 1, imdb_id, overview FROM titles WHERE imdb_id IS NOT NULL AND overview IS NOT NULL
                     UNION ALL SELECT 0, id, overview FROM ext_titles WHERE detailed > 0 AND overview IS NOT NULL",
                )?;
                let r = st.query_map([], |r| Ok((r.get::<_, i64>(0)? == 1, r.get(1)?, r.get(2)?)))?;
                r.collect()
            })
            .unwrap_or_default();
        let next =
            todo.into_iter().find(|(_, id, o)| o.as_deref().is_some_and(|o| !has_cyrillic(o)) && !tried.contains(id));
        let Some((local, id, _)) = next else {
            let _ = tokio::time::timeout(Duration::from_secs(3600), app.discover_kick.notified()).await;
            continue;
        };
        if let Some(ru) = overview(&app.http, &id).await {
            let sql = if local {
                "UPDATE titles SET overview = ?1 WHERE imdb_id = ?2"
            } else {
                "UPDATE ext_titles SET overview = ?1 WHERE id = ?2"
            };
            let _ = app.db.with(|c| c.execute(sql, params![ru, id]));
        } else {
            let mut t = tried;
            t.push(id);
            let keep: Vec<String> = t.into_iter().rev().take(5000).collect();
            let _ = app.db.set_setting("ruwiki.tried", &serde_json::to_string(&keep).unwrap_or_default());
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_and_trim() {
        let full = "Вступление.\n\n== Сюжет ==\n=== Первый сезон ===\nУолтер - учитель химии. Он болен.\n\n== Роли ==\nКто-то.";
        assert_eq!(plot_section(full).as_deref(), Some("Уолтер - учитель химии. Он болен."));
        assert_eq!(plot_section("Без сюжета."), None);
        let long = "Предложение. ".repeat(100);
        let t = trim(&long);
        assert!(t.chars().count() <= MAX && t.ends_with('.'));
        assert!(has_cyrillic("Привет") && !has_cyrillic("Hello"));
    }
}
