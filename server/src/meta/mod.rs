//! Описание фильмов и сериалов: поиск по названию из имени файла, постеры, фоны, актёры, серии.
//! По умолчанию — IMDb без ключа; можно TMDB или Кинопоиск (с ключом, описания на русском).

pub mod imdb;
pub mod kp;
pub mod tmdb;

use crate::util::now_ms;
use crate::App;
use futures_util::{stream, StreamExt};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub provider: &'static str,
    pub id: String,
    pub title: String,
    pub original: Option<String>,
    pub year: Option<i32>,
    pub series: bool,
    pub overview: Option<String>,
    pub poster: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub role: Option<String>,
    /// director, writer, creator, actor
    pub job: String,
    pub photo: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Episode {
    pub season: i32,
    pub episode: i32,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub still: Option<String>,
    pub air_date: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Details {
    pub title: String,
    pub original: Option<String>,
    pub year: Option<i32>,
    pub series: bool,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub genres: Vec<String>,
    pub countries: Vec<String>,
    pub languages: Vec<String>,
    pub aka: Vec<String>,
    pub people: Vec<Person>,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
    pub runtime: Option<i64>,
    pub age: Option<String>,
    pub release_date: Option<String>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub logo: Option<String>,
    pub gallery: Vec<String>,
    pub imdb_id: Option<String>,
    pub episodes: Vec<Episode>,
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Жанры IMDb по-русски (у IMDb они только на английском).
pub fn genre_ru(g: &str) -> String {
    let ru = match g {
        "Action" => "Боевик",
        "Adventure" => "Приключения",
        "Animation" => "Мультфильм",
        "Biography" => "Биография",
        "Comedy" => "Комедия",
        "Crime" => "Криминал",
        "Documentary" => "Документальный",
        "Drama" => "Драма",
        "Family" => "Семейный",
        "Fantasy" => "Фэнтези",
        "Film-Noir" => "Нуар",
        "History" => "История",
        "Horror" => "Ужасы",
        "Music" => "Музыка",
        "Musical" => "Мюзикл",
        "Mystery" => "Детектив",
        "Romance" => "Мелодрама",
        "Sci-Fi" => "Фантастика",
        "Short" => "Короткометражка",
        "Sport" => "Спорт",
        "Thriller" => "Триллер",
        "War" => "Военный",
        "Western" => "Вестерн",
        "Reality-TV" => "Реалити-шоу",
        "Talk-Show" => "Ток-шоу",
        "Game-Show" => "Игра",
        "News" => "Новости",
        other => other,
    };
    ru.to_string()
}

pub enum Provider {
    Imdb(imdb::Imdb),
    Tmdb(tmdb::Tmdb),
    Kp(kp::Kp),
}

impl Provider {
    /// Источник из настроек; `None` — описания выключены или нет ключа.
    pub fn from_settings(app: &App) -> Option<Provider> {
        let db = &app.db;
        let http = app.http.clone();
        match db.setting_or("meta.provider", "imdb").as_str() {
            "imdb" => Some(Provider::Imdb(imdb::Imdb::new(http))),
            "tmdb" => {
                let key = db.setting("meta.tmdb.key").filter(|k| !k.is_empty())?;
                Some(Provider::Tmdb(tmdb::Tmdb {
                    key,
                    api: db.setting_or("meta.tmdb.api", "https://api.themoviedb.org/3"),
                    images: db.setting_or("meta.tmdb.images", "https://image.tmdb.org/t/p"),
                    http,
                }))
            }
            "kinopoisk" => {
                let key = db.setting("meta.kp.key").filter(|k| !k.is_empty())?;
                Some(Provider::Kp(kp::Kp { key, api: db.setting_or("meta.kp.api", "https://api.kinopoisk.dev"), http }))
            }
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Provider::Imdb(_) => "imdb",
            Provider::Tmdb(_) => "tmdb",
            Provider::Kp(_) => "kinopoisk",
        }
    }

    pub async fn search(&self, q: &str, year: Option<i32>, series: bool) -> anyhow::Result<Vec<Candidate>> {
        match self {
            Provider::Imdb(p) => p.search(q, year, series).await,
            Provider::Tmdb(p) => {
                let mut r = p.search(q, year, series).await?;
                if r.is_empty() && year.is_some() {
                    r = p.search(q, None, series).await?;
                }
                Ok(r)
            }
            Provider::Kp(p) => p.search(q, year, series).await,
        }
    }

    pub async fn details(&self, id: &str, seasons: &[i32]) -> anyhow::Result<Details> {
        match self {
            Provider::Imdb(p) => p.details(id, seasons).await,
            Provider::Tmdb(p) => p.details(id, seasons).await,
            Provider::Kp(p) => p.details(id, seasons).await,
        }
    }
}

/// Лучший кандидат: тот же вид (фильм/сериал) и год ±1; без года — первый того же вида.
pub fn best(cands: &[Candidate], year: Option<i32>, series: bool) -> Option<&Candidate> {
    cands.iter().find(|c| {
        c.series == series
            && match (year, c.year) {
                (Some(y), Some(cy)) => (y - cy).abs() <= 1,
                (Some(_), None) => false,
                (None, _) => true,
            }
    })
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct MetaState {
    pub running: bool,
    pub error: Option<String>,
    pub done: usize,
}

fn seasons_of(app: &App, title_id: i64) -> Vec<i32> {
    app.db
        .with(|c| {
            let mut st =
                c.prepare("SELECT DISTINCT season FROM files WHERE title_id = ? AND season IS NOT NULL ORDER BY 1")?;
            let r = st.query_map([title_id], |r| r.get(0))?;
            r.collect()
        })
        .unwrap_or_default()
}

/// Скачать картинки и записать описание. `status`: matched — найдено само, manual — выбрал хост.
pub async fn apply(
    app: &App,
    title_id: i64,
    provider: &str,
    provider_id: &str,
    d: Details,
    status: &str,
) -> anyhow::Result<()> {
    let im = &app.images;
    let get = |u: Option<String>| async move {
        match u {
            Some(u) => im.download(&u).await.map_err(|e| tracing::debug!(error = %e, "картинка не скачалась")).ok(),
            None => None,
        }
    };
    let (poster, backdrop, logo) = tokio::join!(get(d.poster.clone()), get(d.backdrop.clone()), get(d.logo.clone()));
    let gallery: Vec<String> =
        stream::iter(d.gallery.clone()).map(|u| get(Some(u))).buffered(4).filter_map(|x| async { x }).collect().await;
    let photos: Vec<Option<String>> =
        stream::iter(d.people.iter().map(|p| p.photo.clone()).collect::<Vec<_>>()).map(get).buffered(4).collect().await;
    let people: Vec<Person> = d.people.iter().zip(photos).map(|(p, ph)| Person { photo: ph, ..p.clone() }).collect();
    let stills: Vec<Option<String>> = stream::iter(d.episodes.iter().map(|e| e.still.clone()).collect::<Vec<_>>())
        .map(get)
        .buffered(4)
        .collect()
        .await;
    let js = |v: &Vec<String>| serde_json::to_string(v).unwrap_or_else(|_| "[]".into());
    let imdb_id = d.imdb_id.clone().or_else(|| (provider == "imdb").then(|| provider_id.to_string()));
    app.db.tx(|tx| {
        tx.execute(
            "UPDATE titles SET title = COALESCE(NULLIF(?1, ''), title), original_title = ?2, year = COALESCE(?3, year),
               overview = ?4, tagline = ?5, genres = ?6, countries = ?7, languages = ?8, aka = ?9, people = ?10,
               rating = ?11, votes = ?12, runtime = ?13, age = ?14, age_min = ?15, release_date = ?16,
               poster_sm = CASE WHEN ?17 IS NOT NULL THEN NULL ELSE poster_sm END, poster = COALESCE(?17, poster),
               backdrop = COALESCE(?18, backdrop), logo = ?19, gallery = ?20, imdb_id = ?21, provider = ?22,
               provider_id = ?23, meta_status = ?24, updated_at = ?25
             WHERE id = ?26",
            params![
                d.title,
                d.original,
                d.year,
                d.overview,
                d.tagline,
                js(&d.genres),
                js(&d.countries),
                js(&d.languages),
                js(&d.aka),
                serde_json::to_string(&people).unwrap_or_else(|_| "[]".into()),
                d.rating,
                d.votes,
                d.runtime,
                d.age,
                d.age.as_deref().and_then(crate::taste::age_min),
                d.release_date,
                poster,
                backdrop,
                logo,
                js(&gallery),
                imdb_id,
                provider,
                provider_id,
                status,
                now_ms(),
                title_id
            ],
        )?;
        for (e, still) in d.episodes.iter().zip(stills) {
            tx.execute(
                "UPDATE files SET name = ?, overview = ?, still = COALESCE(?, still), air_date = ?
                 WHERE title_id = ? AND season = ? AND episode = ?",
                params![e.name, e.overview, still, e.air_date, title_id, e.season, e.episode],
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

/// Найти и записать описание одного произведения. Ok(false) — ничего подходящего не нашлось.
pub async fn fetch_one(app: &App, p: &Provider, id: i64) -> anyhow::Result<bool> {
    let (title, year, kind): (String, Option<i32>, String) = app.db.with(|c| {
        c.query_row("SELECT title, year, kind FROM titles WHERE id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
    })?;
    let series = kind == "series";
    let cands = p.search(&title, year, series).await?;
    let Some(c) = best(&cands, year, series) else {
        app.db.with(|c| {
            c.execute("UPDATE titles SET meta_status = 'none', updated_at = ? WHERE id = ?", params![now_ms(), id])
        })?;
        return Ok(false);
    };
    let d = p.details(&c.id, &seasons_of(app, id)).await?;
    apply(app, id, p.name(), &c.id, d, "matched").await?;
    Ok(true)
}

/// Выбор хоста из результатов поиска.
pub async fn match_manual(app: &App, id: i64, provider_id: &str) -> anyhow::Result<()> {
    let p = Provider::from_settings(app).ok_or_else(|| anyhow::anyhow!("Источник описаний не настроен"))?;
    let d = p.details(provider_id, &seasons_of(app, id)).await?;
    apply(app, id, p.name(), provider_id, d, "manual").await
}

/// Фоновая работа: по очереди ищет описания для новых произведений.
pub async fn worker(app: Arc<App>) {
    loop {
        let pending: Vec<i64> = app
            .db
            .with(|c| {
                let mut st = c.prepare(
                    "SELECT id FROM titles WHERE meta_status = 'pending' AND locked = 0 ORDER BY added_at DESC LIMIT 200",
                )?;
                let r = st.query_map([], |r| r.get(0))?;
                r.collect()
            })
            .unwrap_or_default();
        let provider = Provider::from_settings(&app);
        let mut failed = false;
        if let (Some(p), false) = (&provider, pending.is_empty()) {
            app.meta_state.lock().running = true;
            for id in pending {
                match fetch_one(&app, p, id).await {
                    Ok(_) => {
                        let mut s = app.meta_state.lock();
                        s.done += 1;
                        s.error = None;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "описание не получено");
                        app.meta_state.lock().error = Some(e.to_string());
                        failed = true;
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(if p.name() == "kinopoisk" { 400 } else { 150 })).await;
            }
            app.meta_state.lock().running = false;
            app.kick_probe();
            if !failed {
                continue;
            }
        }
        // Нет работы или сеть недоступна: ждём новых файлов, смены настроек или 10 минут.
        let wait = if failed { 600 } else { 3600 };
        let _ = tokio::time::timeout(Duration::from_secs(wait), app.meta_kick.notified()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: &str, year: Option<i32>, series: bool) -> Candidate {
        Candidate {
            provider: "imdb",
            id: id.into(),
            title: id.into(),
            original: None,
            year,
            series,
            overview: None,
            poster: None,
        }
    }

    #[test]
    fn picks_same_kind_and_close_year() {
        let list = vec![c("a", Some(2000), true), c("b", Some(2011), false), c("c", Some(2010), false)];
        assert_eq!(best(&list, Some(2010), false).unwrap().id, "b");
        assert_eq!(best(&list, None, true).unwrap().id, "a");
        assert!(best(&list, Some(1990), false).is_none());
    }
}
