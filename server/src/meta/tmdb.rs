//! TMDB (themoviedb.org): поиск и описание на русском, постеры, фоны, логотипы, серии.
//! Ключ API v3 (32 знака) или токен чтения v4 (`eyJ…`). Адреса API и картинок можно сменить
//! на зеркало - в настройках.

use super::{Candidate, Details, Episode, Person};
use serde_json::Value;

pub struct Tmdb {
    pub key: String,
    pub api: String,
    pub images: String,
    pub http: reqwest::Client,
}

fn year_of(date: &Value) -> Option<i32> {
    date.as_str().and_then(|d| d.get(..4)).and_then(|y| y.parse().ok())
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

impl Tmdb {
    async fn get(&self, path: &str, query: &[(&str, String)]) -> anyhow::Result<Value> {
        let mut req = self.http.get(format!("{}{path}", self.api.trim_end_matches('/'))).query(query);
        if self.key.starts_with("eyJ") {
            req = req.bearer_auth(&self.key);
        } else {
            req = req.query(&[("api_key", &self.key)]);
        }
        let res = req.send().await?;
        match res.status().as_u16() {
            401 => anyhow::bail!("TMDB не принял ключ API - проверьте его в настройках"),
            404 => anyhow::bail!("TMDB: не найдено"),
            s if s >= 400 => anyhow::bail!("TMDB ответил {s}"),
            _ => Ok(res.json().await?),
        }
    }

    fn img(&self, size: &str, path: &Value) -> Option<String> {
        path.as_str().map(|p| format!("{}/{size}{p}", self.images.trim_end_matches('/')))
    }

    /// Постер на русском (или без языка, или английский) по id IMDb - для ленты Обзора. `None` - не нашли.
    pub async fn poster_ru(&self, imdb: &str) -> anyhow::Result<Option<String>> {
        let found = self.get(&format!("/find/{imdb}"), &[("external_source", "imdb_id".into())]).await?;
        let (kind, id) = match (found["movie_results"][0]["id"].as_i64(), found["tv_results"][0]["id"].as_i64()) {
            (Some(id), _) => ("movie", id),
            (None, Some(id)) => ("tv", id),
            _ => return Ok(None),
        };
        let images =
            self.get(&format!("/{kind}/{id}/images"), &[("include_image_language", "ru,null,en".into())]).await?;
        let posters = images["posters"].as_array().cloned().unwrap_or_default();
        let best = ["ru", "", "en"].iter().find_map(|lang| {
            posters
                .iter()
                .filter(|p| p["iso_639_1"].as_str().unwrap_or("") == *lang)
                .max_by(|a, b| a["vote_count"].as_i64().cmp(&b["vote_count"].as_i64()))
        });
        Ok(best.and_then(|p| self.img("w500", &p["file_path"])))
    }

    pub async fn search(&self, query: &str, year: Option<i32>, series: bool) -> anyhow::Result<Vec<Candidate>> {
        let kind = if series { "tv" } else { "movie" };
        let mut q = vec![("query", query.to_string()), ("language", "ru-RU".into()), ("include_adult", "false".into())];
        if let Some(y) = year {
            q.push((if series { "first_air_date_year" } else { "year" }, y.to_string()));
        }
        let v = self.get(&format!("/search/{kind}"), &q).await?;
        Ok(v["results"]
            .as_array()
            .into_iter()
            .flatten()
            .take(12)
            .map(|r| Candidate {
                provider: "tmdb",
                id: format!("{kind}/{}", r["id"]),
                title: text(&r[if series { "name" } else { "title" }]).unwrap_or_default(),
                original: text(&r[if series { "original_name" } else { "original_title" }]),
                year: year_of(&r[if series { "first_air_date" } else { "release_date" }]),
                series,
                overview: text(&r["overview"]),
                poster: self.img("w342", &r["poster_path"]),
            })
            .collect())
    }

    /// `id` - «movie/123» или «tv/456»; `seasons` - какие сезоны есть в файлах.
    pub async fn details(&self, id: &str, seasons: &[i32]) -> anyhow::Result<Details> {
        let series = id.starts_with("tv/");
        let ru = [
            ("language", "ru-RU".to_string()),
            (
                "append_to_response",
                if series { "credits,images,content_ratings" } else { "credits,images,release_dates" }.into(),
            ),
            ("include_image_language", "ru,en,null".into()),
        ];
        let v = self.get(&format!("/{id}"), &ru).await?;
        let mut overview = text(&v["overview"]);
        if overview.is_none() {
            let en = self.get(&format!("/{id}"), &[("language", "en-US".to_string())]).await.ok();
            overview = en.and_then(|e| text(&e["overview"]));
        }
        let logos = v["images"]["logos"].as_array().cloned().unwrap_or_default();
        let logo = ["ru", "en"]
            .iter()
            .find_map(|l| logos.iter().find(|x| x["iso_639_1"] == *l))
            .and_then(|x| self.img("w500", &x["file_path"]));
        let mut people: Vec<Person> = v["credits"]["cast"]
            .as_array()
            .into_iter()
            .flatten()
            .take(12)
            .map(|c| Person {
                name: text(&c["name"]).unwrap_or_default(),
                role: text(&c["character"]),
                job: "actor".into(),
                photo: self.img("w185", &c["profile_path"]),
                id: None,
            })
            .collect();
        let directors: Vec<Person> = if series {
            v["created_by"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| text(&c["name"]))
                .map(|name| Person { name, role: None, job: "creator".into(), photo: None, id: None })
                .collect()
        } else {
            v["credits"]["crew"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["job"] == "Director")
                .filter_map(|c| text(&c["name"]))
                .map(|name| Person { name, role: None, job: "director".into(), photo: None, id: None })
                .collect()
        };
        people.splice(0..0, directors);
        let age = if series {
            v["content_ratings"]["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|r| r["iso_3166_1"] == "RU")
                .or_else(|| {
                    v["content_ratings"]["results"].as_array().and_then(|a| a.iter().find(|r| r["iso_3166_1"] == "US"))
                })
                .and_then(|r| text(&r["rating"]))
        } else {
            v["release_dates"]["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|r| r["iso_3166_1"] == "RU")
                .or_else(|| {
                    v["release_dates"]["results"].as_array().and_then(|a| a.iter().find(|r| r["iso_3166_1"] == "US"))
                })
                .and_then(|r| r["release_dates"].as_array()?.iter().find_map(|d| text(&d["certification"])))
        };
        let mut episodes = Vec::new();
        if series {
            for s in seasons {
                let Ok(sv) = self.get(&format!("/{id}/season/{s}"), &[("language", "ru-RU".to_string())]).await else {
                    continue;
                };
                for e in sv["episodes"].as_array().into_iter().flatten() {
                    episodes.push(Episode {
                        season: *s,
                        episode: e["episode_number"].as_i64().unwrap_or(0) as i32,
                        name: text(&e["name"]),
                        overview: text(&e["overview"]),
                        still: self.img("w300", &e["still_path"]),
                        air_date: text(&e["air_date"]),
                        ..Default::default()
                    });
                }
            }
        }
        let names = |key: &str| -> Vec<String> {
            v[key].as_array().into_iter().flatten().filter_map(|g| text(&g["name"])).collect()
        };
        Ok(Details {
            title: text(&v[if series { "name" } else { "title" }]).unwrap_or_default(),
            original: text(&v[if series { "original_name" } else { "original_title" }]),
            year: year_of(&v[if series { "first_air_date" } else { "release_date" }]),
            series,
            overview,
            tagline: text(&v["tagline"]),
            genres: names("genres").into_iter().map(|g| super::capitalize(&g)).collect(),
            countries: if series {
                v["origin_country"].as_array().into_iter().flatten().filter_map(text).collect()
            } else {
                names("production_countries")
            },
            people,
            rating: v["vote_average"].as_f64().filter(|r| *r > 0.0),
            runtime: v["runtime"].as_i64().or_else(|| v["episode_run_time"][0].as_i64()),
            age,
            poster: self.img("w500", &v["poster_path"]),
            backdrop: self.img("w1280", &v["backdrop_path"]),
            logo,
            episodes,
            ..Default::default()
        })
    }
}
