//! Люди кино (любимые режиссёры, актёры у фильма): фото и фильмография с IMDb. Кто это на IMDb - по id из описаний фильмов
//! (у новых описаний он есть), иначе поиском IMDb по имени. Фото и постеры - к себе, раз в неделю.

use crate::discover::{parse_chart, posters_of, upsert};
use crate::meta::imdb::{sized, Imdb};
use crate::util::now_ms;
use crate::App;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

const WEEK: i64 = 7 * 86_400_000;

const SEARCH: &str = r#"query S($t: String!) { mainSearch(first: 3, options: { searchTerm: $t, type: NAME, includeAdult: false }) {
  edges { node { entity { ... on Name { id nameText { text } } } } } } }"#;

const NAME: &str = r#"query N($id: ID!, $cats: [ID!]) { name(id: $id) { nameText { text } primaryImage { url }
  credits(first: 80, filter: { categories: $cats }) { edges { node { title {
    id titleText { text } originalTitleText { text } titleType { id isSeries }
    releaseYear { year } ratingsSummary { aggregateRating voteCount }
    primaryImage { url } genres { genres { text } }
  } } } } } }"#;

/// Id режиссёра у нас уже известен (описания фильмов или прошлый поиск).
fn known_id(app: &App, name: &str) -> Option<String> {
    app.db
        .with(|c| {
            c.query_row(
                "SELECT id FROM ext_people_names WHERE name = ?1
                 UNION ALL SELECT json_extract(p.value, '$.id') FROM titles t, json_each(t.people) p
                   WHERE json_extract(p.value, '$.name') = ?1 AND json_extract(p.value, '$.id') IS NOT NULL
                 UNION ALL SELECT json_extract(p.value, '$.id') FROM ext_titles e, json_each(json_extract(e.extra, '$.people')) p
                   WHERE e.extra IS NOT NULL AND json_extract(p.value, '$.name') = ?1 AND json_extract(p.value, '$.id') IS NOT NULL
                 LIMIT 1",
                [name],
                |r| r.get::<_, String>(0),
            )
            .optional()
        })
        .ok()
        .flatten()
}

/// Кто это на IMDb (nm…): из своих данных или поиском по имени. Найденное запоминаем.
pub async fn resolve(app: &App, name: &str) -> anyhow::Result<Option<String>> {
    if let Some(id) = known_id(app, name) {
        return Ok(Some(id));
    }
    let imdb = Imdb::new(app.http.clone());
    let v = tokio::time::timeout(Duration::from_secs(4), imdb.data(SEARCH, json!({ "t": name }), "mainSearch"))
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(Value::Null);
    let mut id = v["edges"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["node"]["entity"]["id"].as_str())
        .find(|i| i.starts_with("nm"))
        .map(String::from);
    if id.is_none() {
        let query = name.to_lowercase().replace(' ', "_");
        let mut url = reqwest::Url::parse("https://v3.sg.media-imdb.com/suggestion/x/")?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Неверный адрес"))?
            .pop_if_empty()
            .push(&format!("{query}.json"));
        if let Ok(Ok(suggestions)) = tokio::time::timeout(Duration::from_secs(3), async {
            app.http.get(url).send().await?.error_for_status()?.json::<Value>().await
        })
        .await
        {
            id = suggestions["d"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|e| e["l"].as_str().is_some_and(|s| crate::util::norm(s) == crate::util::norm(name)))
                .filter_map(|e| e["id"].as_str())
                .find(|s| s.starts_with("nm"))
                .map(String::from);
        }
    }
    if id.is_none() {
        if let Ok(Ok(found)) = tokio::time::timeout(Duration::from_secs(6), async {
            let found: Value = app
                .http
                .get("https://www.wikidata.org/w/api.php")
                .query(&[
                    ("action", "wbsearchentities"),
                    ("search", name),
                    ("language", "ru"),
                    ("format", "json"),
                    ("limit", "5"),
                ])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let qid = found["search"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|e| {
                    [e["label"].as_str(), e["match"]["text"].as_str()]
                        .into_iter()
                        .flatten()
                        .any(|s| crate::util::norm(s) == crate::util::norm(name))
                })
                .and_then(|e| e["id"].as_str());
            let Some(qid) = qid else {
                return Ok::<Option<String>, reqwest::Error>(None);
            };
            let entity: Value = app
                .http
                .get("https://www.wikidata.org/w/api.php")
                .query(&[("action", "wbgetentities"), ("ids", qid), ("props", "claims"), ("format", "json")])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(entity["entities"][qid]["claims"]["P345"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["mainsnak"]["datavalue"]["value"].as_str())
                .find(|s| s.starts_with("nm") && s[2..].bytes().all(|b| b.is_ascii_digit()))
                .map(String::from))
        })
        .await
        {
            id = found;
        }
    }
    if let Some(id) = &id {
        app.db.with(|c| {
            c.execute(
                "INSERT INTO ext_people_names (name, id, at) VALUES (?, ?, ?)
                 ON CONFLICT(name) DO UPDATE SET id = excluded.id, at = excluded.at",
                params![name, id, now_ms()],
            )
        })?;
    }
    Ok(id)
}

/// Роль: режиссёр или актёр (у актрис на IMDb своя категория).
pub fn role(r: &str) -> &'static str {
    if r == "actor" {
        "actor"
    } else {
        "director"
    }
}

/// Фото и фильмография человека в этой роли - с IMDb не чаще раза в неделю.
pub async fn fetch(app: &Arc<App>, id: &str, role: &str) -> anyhow::Result<()> {
    let role = self::role(role);
    let at: Option<i64> = app.db.with(|c| {
        c.query_row("SELECT at FROM ext_people WHERE id = ? AND role = ?", params![id, role], |r| r.get(0)).optional()
    })?;
    let has_works: bool = app.db.with(|c| {
        c.query_row(
            "SELECT EXISTS(SELECT 1 FROM ext_people WHERE id = ? AND role = ? AND works != '[]')",
            params![id, role],
            |r| r.get(0),
        )
    })?;
    if has_works && at.is_some_and(|at| now_ms() - at < WEEK) {
        return Ok(());
    }
    let cats: &[&str] = if role == "actor" { &["actor", "actress"] } else { &["director"] };
    let v = Imdb::new(app.http.clone()).data(NAME, json!({ "id": id, "cats": cats }), "name").await?;
    let edges: Vec<Value> =
        v["credits"]["edges"].as_array().into_iter().flatten().map(|e| json!({ "node": e["node"]["title"] })).collect();
    let mut works = parse_chart(&json!({ "edges": edges }));
    works.sort_by_key(|w| std::cmp::Reverse(w.year.unwrap_or(0)));
    works.dedup_by(|a, b| a.id == b.id);
    upsert(app, &works)?;
    let ids: Vec<String> = works.iter().map(|w| w.id.clone()).collect();
    let photo_url = v["primaryImage"]["url"].as_str().map(|u| sized(u, 400));
    let (photo, _) = tokio::join!(
        async {
            match photo_url {
                Some(u) => tokio::time::timeout(Duration::from_secs(8), app.images.download(&u)).await.ok()?.ok(),
                None => None,
            }
        },
        posters_of(app, &ids),
    );
    let name = v["nameText"]["text"].as_str().unwrap_or_default().to_string();
    app.db.with(|c| {
        c.execute(
            "INSERT INTO ext_people (id, role, name, photo, works, at) VALUES (?1, ?6, ?2, ?3, ?4, ?5)
             ON CONFLICT(id, role) DO UPDATE SET name = ?2, photo = COALESCE(?3, photo), works = ?4, at = ?5",
            params![id, name, photo, serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into()), now_ms(), role],
        )
    })?;
    Ok(())
}

/// Независимый от IMDb источник: совпадение личности проверяется по IMDb ID в Викиданных.
/// Биография берётся только из русской статьи, факты - из структурированных утверждений.
pub async fn information(app: &Arc<App>, id: &str, name: &str, role: &str) -> anyhow::Result<Value> {
    let key = format!("{id}:{role}");
    let cached: Option<String> = app.db.with(|c| {
        c.query_row("SELECT data FROM person_info WHERE id = ? AND at > ?", params![key, now_ms() - WEEK], |r| r.get(0))
            .optional()
    })?;
    if let Some(raw) = cached {
        let data: Value = serde_json::from_str(&raw)?;
        if data["works"].as_array().is_some_and(|w| !w.is_empty()) {
            return Ok(data);
        }
    }
    let http = &app.http;
    let search: Value = http
        .get("https://www.wikidata.org/w/api.php")
        .query(&[
            ("action", "wbsearchentities"),
            ("search", name),
            ("language", "en"),
            ("limit", "5"),
            ("format", "json"),
        ])
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let ids = search["search"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["id"].as_str())
        .collect::<Vec<_>>()
        .join("|");
    anyhow::ensure!(!ids.is_empty(), "Человек не найден в Викиданных");
    let entities: Value = http
        .get("https://www.wikidata.org/w/api.php")
        .query(&[
            ("action", "wbgetentities"),
            ("ids", ids.as_str()),
            ("props", "claims|labels|descriptions|sitelinks"),
            ("languages", "ru|en"),
            ("format", "json"),
        ])
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let (qid, entity) = entities["entities"]
        .as_object()
        .into_iter()
        .flatten()
        .find(|(_, e)| {
            e["claims"]["P345"].as_array().is_some_and(|a| a.iter().any(|v| v["mainsnak"]["datavalue"]["value"] == id))
        })
        .ok_or_else(|| anyhow::anyhow!("Не удалось подтвердить личность по IMDb ID"))?;
    anyhow::ensure!(qid.starts_with('Q') && qid[1..].bytes().all(|b| b.is_ascii_digit()), "Неверный ID");
    let wiki = entity["sitelinks"]["ruwiki"]["title"].as_str();
    let property = if role == "actor" { "P161" } else { "P57" };
    let query = format!(
        r#"SELECT ?imdb (SAMPLE(?ru) AS ?title) (SAMPLE(?en) AS ?original) (MIN(?released) AS ?date) (SAMPLE(?type) AS ?kind) WHERE {{
      ?film wdt:{property} wd:{qid}; wdt:P345 ?imdb.
      OPTIONAL {{ ?film rdfs:label ?ru FILTER(LANG(?ru)="ru") }} OPTIONAL {{ ?film rdfs:label ?en FILTER(LANG(?en)="en") }}
      OPTIONAL {{ ?film wdt:P577 ?released }} BIND(IF(EXISTS {{ ?film wdt:P31/wdt:P279* wd:Q5398426 }}, "series", "movie") AS ?type)
    }} GROUP BY ?imdb LIMIT 80"#
    );
    let (summary, filmography) = tokio::join!(
        async {
            let title = wiki?;
            let mut url = reqwest::Url::parse("https://ru.wikipedia.org/api/rest_v1/page/summary/").ok()?;
            url.path_segments_mut().ok()?.pop_if_empty().push(title);
            http.get(url)
                .timeout(Duration::from_secs(6))
                .send()
                .await
                .ok()?
                .error_for_status()
                .ok()?
                .json::<Value>()
                .await
                .ok()
        },
        async {
            http.get("https://query.wikidata.org/sparql")
                .query(&[("format", "json"), ("query", query.as_str())])
                .header("user-agent", "Nova/1.0 (home cinema)")
                .timeout(Duration::from_secs(8))
                .send()
                .await?
                .error_for_status()?
                .json::<Value>()
                .await
        }
    );
    let summary = summary.unwrap_or(Value::Null);
    let mut works = Vec::new();
    if let Ok(films) = filmography {
        for b in films["results"]["bindings"].as_array().into_iter().flatten() {
            let Some(fid) = b["imdb"]["value"].as_str().filter(|i| crate::discover::valid_id(i)) else {
                continue;
            };
            let original = b["original"]["value"].as_str();
            let Some(title) = b["title"]["value"].as_str().or(original) else {
                continue;
            };
            works.push(crate::discover::Ext {
                id: fid.into(),
                series: b["kind"]["value"] == "series",
                title: title.into(),
                original: original.map(String::from),
                year: b["date"]["value"].as_str().and_then(|s| s.get(..4)).and_then(|y| y.parse().ok()),
                rating: None,
                votes: None,
                genres: vec![],
                poster_url: None,
            });
        }
    }
    works.sort_by_key(|w| std::cmp::Reverse(w.year.unwrap_or(0)));
    let missing = works
        .iter()
        .filter(|w| {
            app.db
                .with(|c| {
                    c.query_row("SELECT NOT EXISTS(SELECT 1 FROM ext_titles WHERE id=?)", [&w.id], |r| {
                        r.get::<_, bool>(0)
                    })
                })
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    upsert(app, &missing)?;
    let mut photo = None;
    if let Some(url) = summary["thumbnail"]["source"].as_str() {
        photo = tokio::time::timeout(Duration::from_secs(4), app.images.download(url)).await.ok().and_then(Result::ok);
    }
    let birth = entity["claims"]["P569"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v["mainsnak"]["datavalue"]["value"]["time"].as_str())
        .and_then(|s| s.trim_start_matches('+').get(..10))
        .map(String::from);
    let data = json!({ "nameRu": entity["labels"]["ru"]["value"], "description": entity["descriptions"]["ru"]["value"],
        "biography": summary["extract"], "birthDate": birth, "photo": photo,
        "source": format!("https://www.wikidata.org/wiki/{qid}"), "wiki": summary["content_urls"]["desktop"]["page"],
        "works": works.iter().map(|w| &w.id).collect::<Vec<_>>() });
    app.db.with(|c| c.execute("INSERT INTO person_info(id,data,at) VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data, at=excluded.at",
        params![key, data.to_string(), now_ms()]))?;
    Ok(data)
}

/// Уже известное о режиссёре по имени: id и фото (без запросов к IMDb).
pub fn cached(app: &App, name: &str) -> Option<(String, Option<String>)> {
    let id = known_id(app, name)?;
    let photo: Option<String> = app
        .db
        .with(|c| {
            c.query_row("SELECT photo FROM ext_people WHERE id = ? AND photo IS NOT NULL LIMIT 1", [&id], |r| r.get(0))
                .optional()
        })
        .ok()
        .flatten()
        .flatten();
    Some((id, photo))
}

/// Режиссёры профиля без фото - найти в фоне (страница профиля покажет их в следующий раз).
pub fn warm(app: &Arc<App>, names: Vec<String>) {
    let missing: Vec<String> = names.into_iter().filter(|n| cached(app, n).is_none_or(|c| c.1.is_none())).collect();
    if missing.is_empty() {
        return;
    }
    let app = app.clone();
    tokio::spawn(async move {
        for n in missing.into_iter().take(5) {
            if let Ok(Some(id)) = resolve(&app, &n).await {
                let _ = fetch(&app, &id, "director").await;
            }
        }
    });
}
