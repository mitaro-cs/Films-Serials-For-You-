//! IMDb без ключа: поиск — подсказки `v3.sg.media-imdb.com`, описание — GraphQL, которым
//! пользуется сам сайт IMDb (`caching.graphql.imdb.com`). Название берём русское из «akas»,
//! если оно есть; сюжет у IMDb обычно на английском.
//!
//! Это неофициальные адреса: если IMDb их поменяет, разбор не падает, а просто находит меньше.
//! Запрос делится на основной и дополнительный: ошибка во втором не теряет первый.

use super::{Candidate, Details, Episode, Person};
use serde_json::{json, Value};

pub struct Imdb {
    pub http: reqwest::Client,
    pub graphql: Vec<String>,
    pub suggest: String,
}

impl Imdb {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            graphql: vec!["https://caching.graphql.imdb.com/".into(), "https://graphql.imdb.com/".into()],
            suggest: "https://v3.sg.media-imdb.com/suggestion".into(),
        }
    }
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

fn edges(v: &Value) -> impl Iterator<Item = &Value> {
    v["edges"].as_array().into_iter().flatten().map(|e| &e["node"])
}

/// Картинка IMDb нужной ширины: `…._V1_.jpg` → `…._V1_QL80_UX500_.jpg`.
pub fn sized(url: &str, width: u32) -> String {
    match url.find("._V1_") {
        Some(i) => format!("{}._V1_QL80_UX{width}_.jpg", &url[..i]),
        None => url.to_string(),
    }
}

fn is_series_type(id: &str) -> Option<bool> {
    match id {
        "movie" | "tvMovie" | "video" | "short" | "tvShort" | "tvSpecial" | "feature" => Some(false),
        "tvSeries" | "tvMiniSeries" => Some(true),
        _ => None,
    }
}

const MAIN: &str = r#"query T($id: ID!) { title(id: $id) {
  id titleText { text } originalTitleText { text } titleType { id isSeries }
  releaseYear { year endYear } releaseDate { day month year }
  plot { plotText { plainText } } ratingsSummary { aggregateRating voteCount }
  genres { genres { text } } runtime { seconds } certificate { rating }
  primaryImage { url width height }
  countriesOfOrigin { countries { id text } } spokenLanguages { spokenLanguages { id text } }
  principalCredits { category { id text } credits { name { nameText { text } primaryImage { url } } } }
} }"#;

const EXTRA: &str = r#"query X($id: ID!) { title(id: $id) {
  taglines(first: 1) { edges { node { text } } }
  akas(first: 80) { edges { node { text country { id } language { id } } } }
  images(first: 40) { edges { node { url width height } } }
  cast: credits(first: 12, filter: { categories: ["actor", "actress"] }) {
    edges { node { name { nameText { text } primaryImage { url } } ... on Cast { characters { name } } } }
  }
  episodes { seasons { value } }
} }"#;

const EPISODES: &str = r#"query E($id: ID!, $season: String!) { title(id: $id) { episodes {
  episodes(first: 250, filter: { includeSeasons: [$season] }) { edges { node {
    titleText { text } plot { plotText { plainText } } primaryImage { url }
    releaseDate { day month year } series { episodeNumber { episodeNumber seasonNumber } }
  } } }
} } }"#;

impl Imdb {
    async fn gql(&self, query: &str, vars: Value) -> anyhow::Result<Value> {
        let mut last = anyhow::anyhow!("IMDb недоступен");
        for url in &self.graphql {
            let res = self
                .http
                .post(url)
                .header("x-imdb-user-country", "RU")
                .header("x-imdb-user-language", "ru-RU")
                .header("x-imdb-client-name", "imdb-web-next-localized")
                .json(&json!({ "query": query, "variables": vars }))
                .send()
                .await;
            match res {
                Ok(r) if r.status().is_success() => {
                    let v: Value = r.json().await?;
                    if v["data"]["title"].is_object() {
                        return Ok(v["data"]["title"].clone());
                    }
                    last = anyhow::anyhow!("IMDb: {}", v["errors"][0]["message"].as_str().unwrap_or("пустой ответ"));
                }
                Ok(r) => last = anyhow::anyhow!("IMDb ответил {}", r.status().as_u16()),
                Err(e) => last = e.into(),
            }
        }
        Err(last)
    }

    pub async fn search(&self, query: &str, year: Option<i32>, series: bool) -> anyhow::Result<Vec<Candidate>> {
        let q = query.trim().to_lowercase();
        let first = q.chars().next().filter(|c| c.is_ascii_alphanumeric()).unwrap_or('x');
        let enc: String = q
            .bytes()
            .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
            .collect();
        let res = self.http.get(format!("{}/{first}/{enc}.json", self.suggest)).send().await?;
        anyhow::ensure!(res.status().is_success(), "IMDb ответил {}", res.status().as_u16());
        let v: Value = res.json().await?;
        let mut out = parse_suggestions(&v);
        out.sort_by_key(|c| (c.series != series, year.is_some_and(|y| c.year.is_none_or(|cy| (cy - y).abs() > 1))));
        Ok(out)
    }

    pub async fn details(&self, id: &str, seasons: &[i32]) -> anyhow::Result<Details> {
        anyhow::ensure!(id.starts_with("tt") && id[2..].bytes().all(|b| b.is_ascii_digit()), "неверный id IMDb");
        let main = self.gql(MAIN, json!({ "id": id })).await?;
        let extra = self.gql(EXTRA, json!({ "id": id })).await.unwrap_or(Value::Null);
        let mut d = parse_details(&main, &extra);
        if d.series {
            for s in seasons {
                if let Ok(v) = self.gql(EPISODES, json!({ "id": id, "season": s.to_string() })).await {
                    d.episodes.extend(parse_episodes(&v));
                }
            }
        }
        Ok(d)
    }
}

pub fn parse_suggestions(v: &Value) -> Vec<Candidate> {
    v["d"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let id = text(&r["id"]).filter(|i| i.starts_with("tt"))?;
            let series = is_series_type(r["qid"].as_str().unwrap_or("movie"))?;
            Some(Candidate {
                provider: "imdb",
                id,
                title: text(&r["l"]).unwrap_or_default(),
                original: None,
                year: r["y"].as_i64().map(|y| y as i32),
                series,
                overview: text(&r["s"]),
                poster: text(&r["i"]["imageUrl"]).map(|u| sized(&u, 342)),
            })
        })
        .collect()
}

fn date(v: &Value) -> Option<String> {
    let y = v["year"].as_i64()?;
    Some(match (v["month"].as_i64(), v["day"].as_i64()) {
        (Some(m), Some(d)) => format!("{y:04}-{m:02}-{d:02}"),
        (Some(m), None) => format!("{y:04}-{m:02}"),
        _ => format!("{y:04}"),
    })
}

pub fn parse_details(t: &Value, x: &Value) -> Details {
    let series = t["titleType"]["isSeries"]
        .as_bool()
        .or_else(|| t["titleType"]["id"].as_str().and_then(is_series_type))
        .unwrap_or(false);
    let original = text(&t["originalTitleText"]["text"]);
    // Русское название: aka для России или на русском; иначе то, что отдал IMDb.
    let akas: Vec<(String, String, String)> = edges(&x["akas"])
        .filter_map(|a| {
            Some((
                text(&a["text"])?,
                a["country"]["id"].as_str().unwrap_or("").into(),
                a["language"]["id"].as_str().unwrap_or("").into(),
            ))
        })
        .collect();
    let ru = akas
        .iter()
        .find(|a| a.1 == "RU" && (a.2.is_empty() || a.2 == "ru"))
        .or_else(|| akas.iter().find(|a| a.2 == "ru"));
    let title = ru.map(|a| a.0.clone()).or_else(|| text(&t["titleText"]["text"])).unwrap_or_default();
    let mut aka: Vec<String> = Vec::new();
    for a in &akas {
        if a.0 != title && Some(&a.0) != original.as_ref() && !aka.contains(&a.0) && aka.len() < 3 {
            aka.push(a.0.clone());
        }
    }

    let mut people = Vec::new();
    for group in t["principalCredits"].as_array().into_iter().flatten() {
        let job = match group["category"]["id"].as_str() {
            Some("director") => "director",
            Some("writer") => "writer",
            Some("creator") => "creator",
            Some("cast") | Some("actor") | Some("actress") => "actor",
            _ => continue,
        };
        for c in group["credits"].as_array().into_iter().flatten() {
            if let Some(name) = text(&c["name"]["nameText"]["text"]) {
                let photo = text(&c["name"]["primaryImage"]["url"]).map(|u| sized(&u, 128));
                people.push(Person { name, role: None, job: job.into(), photo });
            }
        }
    }
    let cast: Vec<Person> = edges(&x["cast"])
        .filter_map(|c| {
            Some(Person {
                name: text(&c["name"]["nameText"]["text"])?,
                role: c["characters"][0]["name"].as_str().map(String::from),
                job: "actor".into(),
                photo: text(&c["name"]["primaryImage"]["url"]).map(|u| sized(&u, 128)),
            })
        })
        .collect();
    if !cast.is_empty() {
        people.retain(|p| p.job != "actor");
        people.extend(cast);
    }

    // Фон и галерея — широкие кадры из «images».
    let mut wide: Vec<(String, i64)> = edges(&x["images"])
        .filter_map(|i| {
            let (w, h) = (i["width"].as_i64()?, i["height"].as_i64()?);
            (w >= 1000 && w * 10 >= h * 14).then(|| text(&i["url"]).map(|u| (u, w))).flatten()
        })
        .collect();
    wide.dedup_by(|a, b| a.0 == b.0);
    let backdrop = wide.first().map(|(u, _)| sized(u, 1600));
    let gallery = wide.iter().skip(1).take(8).map(|(u, _)| sized(u, 640)).collect();

    Details {
        title,
        original,
        year: t["releaseYear"]["year"].as_i64().map(|y| y as i32),
        series,
        overview: text(&t["plot"]["plotText"]["plainText"]),
        tagline: edges(&x["taglines"]).find_map(|n| text(&n["text"])),
        genres: t["genres"]["genres"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| text(&g["text"]))
            .map(|g| super::genre_ru(&g))
            .collect(),
        countries: t["countriesOfOrigin"]["countries"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| text(&c["text"]))
            .collect(),
        languages: t["spokenLanguages"]["spokenLanguages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| text(&c["text"]))
            .collect(),
        people,
        rating: t["ratingsSummary"]["aggregateRating"].as_f64(),
        votes: t["ratingsSummary"]["voteCount"].as_i64(),
        runtime: t["runtime"]["seconds"].as_i64().map(|s| s / 60).filter(|m| *m > 0),
        age: text(&t["certificate"]["rating"]),
        release_date: date(&t["releaseDate"]),
        poster: text(&t["primaryImage"]["url"]).map(|u| sized(&u, 600)),
        backdrop,
        gallery,
        aka,
        imdb_id: text(&t["id"]),
        ..Default::default()
    }
}

pub fn parse_episodes(t: &Value) -> Vec<Episode> {
    edges(&t["episodes"]["episodes"])
        .filter_map(|e| {
            let n = &e["series"]["episodeNumber"];
            Some(Episode {
                season: n["seasonNumber"].as_i64()? as i32,
                episode: n["episodeNumber"].as_i64()? as i32,
                name: text(&e["titleText"]["text"]).filter(|t| !t.starts_with("Episode #")),
                overview: text(&e["plot"]["plotText"]["plainText"]),
                still: text(&e["primaryImage"]["url"]).map(|u| sized(&u, 400)),
                air_date: date(&e["releaseDate"]),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions() {
        let v = json!({"d": [
            {"i": {"imageUrl": "https://m.media-amazon.com/images/M/abc._V1_.jpg"}, "id": "tt3783958", "l": "La La Land", "qid": "movie", "s": "Ryan Gosling, Emma Stone", "y": 2016},
            {"id": "nm0331516", "l": "Ryan Gosling"},
            {"id": "tt0903747", "l": "Breaking Bad", "qid": "tvSeries", "y": 2008},
            {"id": "tt0959621", "l": "Pilot", "qid": "tvEpisode", "y": 2008}
        ]});
        let c = parse_suggestions(&v);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].poster.as_deref(), Some("https://m.media-amazon.com/images/M/abc._V1_QL80_UX342_.jpg"));
        assert!(c[1].series);
    }

    #[test]
    fn details_prefer_russian_aka() {
        let t = json!({
            "id": "tt3783958", "titleText": {"text": "La La Land"}, "originalTitleText": {"text": "La La Land"},
            "titleType": {"id": "movie", "isSeries": false}, "releaseYear": {"year": 2016},
            "releaseDate": {"day": 25, "month": 12, "year": 2016},
            "plot": {"plotText": {"plainText": "A jazz pianist falls for an aspiring actress."}},
            "ratingsSummary": {"aggregateRating": 8.0, "voteCount": 700000},
            "genres": {"genres": [{"text": "Comedy"}, {"text": "Drama"}]},
            "runtime": {"seconds": 7680}, "certificate": {"rating": "PG-13"},
            "primaryImage": {"url": "https://m.media-amazon.com/images/M/p._V1_.jpg"},
            "principalCredits": [
                {"category": {"id": "director"}, "credits": [{"name": {"nameText": {"text": "Damien Chazelle"}}}]},
                {"category": {"id": "cast"}, "credits": [{"name": {"nameText": {"text": "Ryan Gosling"}}}]}
            ]
        });
        let x = json!({
            "akas": {"edges": [{"node": {"text": "Ла-Ла Ленд", "country": {"id": "RU"}, "language": null}}]},
            "images": {"edges": [
                {"node": {"url": "https://m.media-amazon.com/images/M/w._V1_.jpg", "width": 2000, "height": 1000}},
                {"node": {"url": "https://m.media-amazon.com/images/M/tall._V1_.jpg", "width": 1000, "height": 1500}}
            ]},
            "cast": {"edges": [{"node": {"name": {"nameText": {"text": "Emma Stone"}}, "characters": [{"name": "Mia"}]}}]}
        });
        let d = parse_details(&t, &x);
        assert_eq!(d.title, "Ла-Ла Ленд");
        assert_eq!(d.aka, Vec::<String>::new());
        assert_eq!(d.runtime, Some(128));
        assert_eq!(d.genres, vec!["Комедия", "Драма"]);
        assert_eq!(d.release_date.as_deref(), Some("2016-12-25"));
        assert!(d.backdrop.unwrap().contains("UX1600"));
        assert_eq!(d.people.iter().filter(|p| p.job == "actor").count(), 1);
        assert_eq!(d.people[1].role.as_deref(), Some("Mia"));
    }

    #[test]
    fn episodes() {
        let v = json!({"episodes": {"episodes": {"edges": [
            {"node": {"titleText": {"text": "Pilot"}, "series": {"episodeNumber": {"episodeNumber": 1, "seasonNumber": 1}}}},
            {"node": {"titleText": {"text": "Episode #1.2"}, "series": {"episodeNumber": {"episodeNumber": 2, "seasonNumber": 1}}}}
        ]}}});
        let e = parse_episodes(&v);
        assert_eq!((e[0].name.as_deref(), e[1].name.as_deref()), (Some("Pilot"), None));
    }
}
