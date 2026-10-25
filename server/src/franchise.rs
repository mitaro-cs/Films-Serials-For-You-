//! Хронология франшиз: «Гарри Поттер», «Стражи Галактики», вселенная Marvel… По id IMDb фильма
//! Викиданные дают серии, где он есть (свойство P179), и все фильмы серии с номером (P1545) и датой
//! выхода. Порядок - по номеру, без номера - по дате. Ответ - на 30 дней в `franchises`; фильмов,
//! которых ещё нет в ленте, - одним запросом к IMDb (названия, постеры).

use crate::discover::{parse_chart, posters_of, upsert, valid_id};
use crate::meta::imdb::Imdb;
use crate::util::now_ms;
use crate::App;
use reqwest::Url;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const MONTH: i64 = 30 * 86_400_000;
/// Серий у одного фильма - не больше (самые узкие: «Стражи Галактики» раньше всей вселенной).
const GROUPS: usize = 3;
/// Фильмов в серии - не больше.
const ITEMS: usize = 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Group {
    pub name: String,
    pub ids: Vec<String>,
}

fn sparql(imdb: &str) -> String {
    format!(
        r#"SELECT ?series ?name ?imdb (SAMPLE(?ord) AS ?o) (MIN(?date) AS ?d) WHERE {{
  ?me wdt:P345 "{imdb}"; wdt:P179 ?series.
  ?film wdt:P179 ?series; wdt:P345 ?imdb.
  OPTIONAL {{ ?film p:P179 ?st. ?st ps:P179 ?series; pq:P1545 ?ord }}
  OPTIONAL {{ ?film wdt:P577 ?date }}
  OPTIONAL {{ ?series rdfs:label ?ru FILTER(LANG(?ru) = "ru") }}
  OPTIONAL {{ ?series rdfs:label ?en FILTER(LANG(?en) = "en") }}
  BIND(COALESCE(?ru, ?en, "") AS ?name)
}} GROUP BY ?series ?name ?imdb"#
    )
}

/// Ответ Викиданных → серии по порядку (узкие первыми, одиночки отброшены).
pub fn parse(v: &Value) -> Vec<Group> {
    // серия → (имя, [(id, номер, дата)])
    type Film = (String, Option<f64>, String);
    let mut by: HashMap<String, (String, Vec<Film>)> = HashMap::new();
    for b in v["results"]["bindings"].as_array().into_iter().flatten() {
        let (Some(series), Some(id)) = (b["series"]["value"].as_str(), b["imdb"]["value"].as_str()) else {
            continue;
        };
        if !valid_id(id) {
            continue;
        }
        let name = b["name"]["value"].as_str().unwrap_or_default();
        let ord = b["o"]["value"].as_str().and_then(|o| o.trim().parse::<f64>().ok());
        let date = b["d"]["value"].as_str().unwrap_or_default().chars().take(10).collect::<String>();
        let e = by.entry(series.to_string()).or_insert_with(|| (name.to_string(), vec![]));
        if !e.1.iter().any(|f| f.0 == id) {
            e.1.push((id.to_string(), ord, date));
        }
    }
    let mut groups: Vec<Group> = by
        .into_values()
        .filter(|(name, films)| films.len() > 1 && !name.is_empty())
        .map(|(name, mut films)| {
            // Без даты (ещё не вышел, не объявлен) - в конец.
            let key = |f: &Film| if f.2.is_empty() { "9999".to_string() } else { f.2.clone() };
            films.sort_by(|a, b| {
                let (x, y) = (a.1.unwrap_or(f64::MAX), b.1.unwrap_or(f64::MAX));
                x.total_cmp(&y).then_with(|| key(a).cmp(&key(b)))
            });
            let mut ch = name.chars();
            let name = ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default();
            Group { name, ids: films.into_iter().take(ITEMS).map(|f| f.0).collect() }
        })
        .collect();
    groups.sort_by(|a, b| a.ids.len().cmp(&b.ids.len()).then_with(|| a.name.cmp(&b.name)));
    groups.dedup_by(|a, b| a.ids == b.ids);
    groups.truncate(GROUPS);
    groups
}

async fn lookup(http: &reqwest::Client, imdb: &str) -> anyhow::Result<Vec<Group>> {
    let url = Url::parse_with_params("https://query.wikidata.org/sparql", &[("query", sparql(imdb))])?;
    let v: Value = http
        .get(url)
        .header("user-agent", concat!("Nova/", env!("CARGO_PKG_VERSION"), " (home cinema)"))
        .header("accept", "application/sparql-results+json")
        .timeout(Duration::from_secs(20))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(parse(&v))
}

const TITLES: &str = r#"query T($ids: [ID!]!) { titles(ids: $ids) {
  id titleText { text } originalTitleText { text } titleType { id isSeries }
  releaseYear { year } ratingsSummary { aggregateRating voteCount }
  primaryImage { url } genres { genres { text } }
} }"#;

/// Фильмов серии нет в ленте - взять с IMDb (до 50 за раз) и постеры к себе.
async fn fill(app: &Arc<App>, ids: &[String]) {
    let missing: Vec<String> = ids
        .iter()
        .filter(|id| {
            app.db
                .with(|c| c.query_row("SELECT 1 FROM ext_titles WHERE id = ?", [id], |_| Ok(())).optional())
                .ok()
                .flatten()
                .is_none()
        })
        .cloned()
        .collect();
    for chunk in missing.chunks(50) {
        let Ok(v) = Imdb::new(app.http.clone()).data(TITLES, json!({ "ids": chunk }), "titles").await else {
            continue;
        };
        let edges: Vec<Value> =
            v.as_array().into_iter().flatten().filter(|t| t.is_object()).map(|t| json!({ "node": t })).collect();
        let _ = upsert(app, &parse_chart(&json!({ "edges": edges })));
    }
    let _ = tokio::time::timeout(Duration::from_secs(10), posters_of(app, ids)).await;
}

/// Серии фильма (из кэша, раз в 30 дней - заново). Ошибка Викиданных - пусто, спросим в следующий раз.
pub async fn of(app: &Arc<App>, imdb: &str) -> Vec<Group> {
    if !valid_id(imdb) {
        return vec![];
    }
    let cached: Option<(String, i64)> = app
        .db
        .with(|c| {
            c.query_row("SELECT data, at FROM franchises WHERE imdb = ?", [imdb], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()
        })
        .ok()
        .flatten();
    if let Some((data, at)) = &cached {
        if now_ms() - at < MONTH {
            return serde_json::from_str(data).unwrap_or_default();
        }
    }
    let groups = match lookup(&app.http, imdb).await {
        Ok(g) => g,
        Err(e) => {
            tracing::debug!("франшиза {imdb}: {e}");
            return cached.and_then(|(d, _)| serde_json::from_str(&d).ok()).unwrap_or_default();
        }
    };
    let all: Vec<String> = groups.iter().flat_map(|g| g.ids.iter().cloned()).collect();
    fill(app, &all).await;
    let data = serde_json::to_string(&groups).unwrap_or_else(|_| "[]".into());
    let _ = app.db.with(|c| {
        c.execute(
            "INSERT INTO franchises (imdb, data, at) VALUES (?1, ?2, ?3)
             ON CONFLICT(imdb) DO UPDATE SET data = ?2, at = ?3",
            params![imdb, data, now_ms()],
        )
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(series: &str, name: &str, id: &str, ord: Option<&str>, date: &str) -> Value {
        let mut b = json!({
            "series": {"value": series}, "name": {"value": name}, "imdb": {"value": id}, "d": {"value": date},
        });
        if let Some(o) = ord {
            b["o"] = json!({"value": o});
        }
        b
    }

    #[test]
    fn order_and_groups() {
        let v = json!({"results": {"bindings": [
            row("Q1", "кинематографическая вселенная Marvel", "tt0000003", Some("10"), "2014-07-31T00:00:00Z"),
            row("Q1", "кинематографическая вселенная Marvel", "tt0000001", Some("1"), "2008-04-30T00:00:00Z"),
            row("Q1", "кинематографическая вселенная Marvel", "tt0000002", Some("2"), "2008-06-13T00:00:00Z"),
            row("Q1", "кинематографическая вселенная Marvel", "tt0000009", None, ""),
            row("Q2", "Стражи Галактики", "tt0000004", None, "2017-05-05T00:00:00Z"),
            row("Q2", "Стражи Галактики", "tt0000003", None, "2014-07-31T00:00:00Z"),
            row("Q3", "Одиночка", "tt0000003", Some("1"), "2014-07-31T00:00:00Z"),
            row("Q4", "Мусор", "nm0000001", Some("1"), ""),
        ]}});
        let g = parse(&v);
        assert_eq!(g.len(), 2, "одиночки и чужие id отброшены");
        assert_eq!(g[0], Group { name: "Стражи Галактики".into(), ids: vec!["tt0000003".into(), "tt0000004".into()] });
        assert_eq!(g[1].name, "Кинематографическая вселенная Marvel");
        assert_eq!(g[1].ids, ["tt0000001", "tt0000002", "tt0000003", "tt0000009"], "по номеру, без даты - в конце");
    }
}
