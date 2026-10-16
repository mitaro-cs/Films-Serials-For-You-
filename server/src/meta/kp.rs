//! Кинопоиск через api.kinopoisk.dev (бесплатный ключ — в их Telegram-боте, 200 запросов в день).
//! Работает из России без зеркал.

use super::{Candidate, Details, Episode, Person};
use serde_json::Value;

pub struct Kp {
    pub key: String,
    pub api: String,
    pub http: reqwest::Client,
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

fn is_series(v: &Value) -> bool {
    v["isSeries"].as_bool().unwrap_or(false)
        || matches!(v["type"].as_str(), Some("tv-series" | "animated-series" | "anime" | "tv-show" | "mini-series"))
}

impl Kp {
    async fn get(&self, path: &str, query: &[(&str, String)]) -> anyhow::Result<Value> {
        let res = self
            .http
            .get(format!("{}{path}", self.api.trim_end_matches('/')))
            .query(query)
            .header("X-API-KEY", &self.key)
            .send()
            .await?;
        match res.status().as_u16() {
            401 => anyhow::bail!("Кинопоиск не принял ключ API — проверьте его в настройках"),
            403 => anyhow::bail!("Кинопоиск: закончился дневной лимит запросов, продолжим завтра"),
            s if s >= 400 => anyhow::bail!("Кинопоиск ответил {s}"),
            _ => Ok(res.json().await?),
        }
    }

    pub async fn search(&self, query: &str, year: Option<i32>, series: bool) -> anyhow::Result<Vec<Candidate>> {
        let v = self
            .get("/v1.4/movie/search", &[("page", "1".into()), ("limit", "15".into()), ("query", query.to_string())])
            .await?;
        let mut out: Vec<Candidate> = v["docs"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|d| Candidate {
                provider: "kinopoisk",
                id: d["id"].to_string(),
                title: text(&d["name"]).or_else(|| text(&d["alternativeName"])).unwrap_or_default(),
                original: text(&d["alternativeName"]).or_else(|| text(&d["enName"])),
                year: d["year"].as_i64().map(|y| y as i32),
                series: is_series(d),
                overview: text(&d["shortDescription"]).or_else(|| text(&d["description"])),
                poster: text(&d["poster"]["previewUrl"]).or_else(|| text(&d["poster"]["url"])),
            })
            .collect();
        // Сначала того же вида и года — так автоматический выбор берёт верное.
        out.sort_by_key(|c| (c.series != series, year.is_some_and(|y| c.year.is_none_or(|cy| (cy - y).abs() > 1))));
        Ok(out)
    }

    pub async fn details(&self, id: &str, seasons: &[i32]) -> anyhow::Result<Details> {
        anyhow::ensure!(id.bytes().all(|b| b.is_ascii_digit()), "неверный id Кинопоиска");
        let v = self.get(&format!("/v1.4/movie/{id}"), &[]).await?;
        let series = is_series(&v);
        let names = |key: &str| -> Vec<String> {
            v[key].as_array().into_iter().flatten().filter_map(|g| text(&g["name"])).collect()
        };
        let mut people = Vec::new();
        for p in v["persons"].as_array().into_iter().flatten() {
            let job = match p["enProfession"].as_str() {
                Some("director") => "director",
                Some("actor") => "actor",
                _ => continue,
            };
            if let Some(name) = text(&p["name"]).or_else(|| text(&p["enName"])) {
                people.push(Person { name, role: text(&p["description"]), job: job.into(), photo: text(&p["photo"]) });
            }
        }
        people.sort_by_key(|p| p.job != "director");
        let mut actors = 0;
        people.retain(|p| {
            p.job != "actor" || {
                actors += 1;
                actors <= 12
            }
        });
        let mut episodes = Vec::new();
        if series && !seasons.is_empty() {
            if let Ok(sv) = self.get("/v1.4/season", &[("movieId", id.to_string()), ("limit", "100".into())]).await {
                for s in sv["docs"].as_array().into_iter().flatten() {
                    let n = s["number"].as_i64().unwrap_or(-1) as i32;
                    if !seasons.contains(&n) {
                        continue;
                    }
                    for e in s["episodes"].as_array().into_iter().flatten() {
                        episodes.push(Episode {
                            season: n,
                            episode: e["number"].as_i64().unwrap_or(0) as i32,
                            name: text(&e["name"]).or_else(|| text(&e["enName"])),
                            overview: text(&e["description"]),
                            still: text(&e["still"]["url"]),
                            air_date: text(&e["airDate"]).map(|d| d.chars().take(10).collect()),
                        });
                    }
                }
            }
        }
        let rating = v["rating"]["kp"]
            .as_f64()
            .filter(|r| *r > 0.0)
            .or_else(|| v["rating"]["imdb"].as_f64().filter(|r| *r > 0.0));
        Ok(Details {
            title: text(&v["name"]).or_else(|| text(&v["alternativeName"])).unwrap_or_default(),
            original: text(&v["alternativeName"]).or_else(|| text(&v["enName"])),
            year: v["year"].as_i64().map(|y| y as i32),
            series,
            overview: text(&v["description"]).or_else(|| text(&v["shortDescription"])),
            tagline: text(&v["slogan"]),
            genres: names("genres").into_iter().map(|g| super::capitalize(&g)).collect(),
            countries: names("countries"),
            people,
            rating,
            runtime: v["movieLength"].as_i64().or_else(|| v["seriesLength"].as_i64()),
            age: v["ageRating"].as_i64().map(|a| format!("{a}+")),
            poster: text(&v["poster"]["url"]),
            backdrop: text(&v["backdrop"]["url"]),
            logo: text(&v["logo"]["url"]),
            episodes,
            ..Default::default()
        })
    }
}
