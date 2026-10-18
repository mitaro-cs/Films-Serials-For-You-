//! Лента «как на IMDb»: подборки (популярное, топ-250) и поиск по всему IMDb — фильмы и сериалы,
//! которых может не быть на компьютере хоста. Их можно отметить «Буду смотреть»/«Просмотрел»,
//! положить в свой список и попросить хоста добавить. Картинки сервер скачивает к себе: браузер
//! ходит только к нам. Подборки обновляются раз в 12 часов, постеры докачиваются в фоне.

use crate::meta::imdb::{sized, Imdb};
use crate::util::now_ms;
use crate::App;
use rusqlite::params;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

/// Подборки: ключ у нас, тип у IMDb, подпись.
pub const CHARTS: &[(&str, &str, &str)] = &[
    ("popular_movies", "MOST_POPULAR_MOVIES", "Популярные фильмы"),
    ("popular_series", "MOST_POPULAR_TV_SHOWS", "Популярные сериалы"),
    ("top_movies", "TOP_RATED_MOVIES", "Топ-250 фильмов"),
    ("top_series", "TOP_RATED_TV_SHOWS", "Лучшие сериалы"),
];

const CHART: &str = r#"query C($chart: ChartTitleType!) { chartTitles(first: 250, chart: { chartType: $chart }) {
  edges { currentRank node {
    id titleText { text } originalTitleText { text } titleType { id isSeries }
    releaseYear { year } ratingsSummary { aggregateRating voteCount }
    primaryImage { url } genres { genres { text } }
  } }
} }"#;

const SIMILAR: &str = r#"query S($id: ID!) { title(id: $id) { moreLikeThisTitles(first: 12) {
  edges { node {
    id titleText { text } originalTitleText { text } titleType { id isSeries }
    releaseYear { year } ratingsSummary { aggregateRating voteCount }
    primaryImage { url } genres { genres { text } }
  } }
} } }"#;

/// Описание фильма из ленты в нынешнем виде (`ext_titles.detailed`): 1 — только сюжет (0.6),
/// 2 — со своей страницей: кадр, актёры, кадры, похожие.
pub const DETAILED: i64 = 2;

const EVERY: Duration = Duration::from_secs(12 * 3600);

#[derive(Debug, Clone, PartialEq)]
pub struct Ext {
    pub id: String,
    pub series: bool,
    pub title: String,
    pub original: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
    pub genres: Vec<String>,
    pub poster_url: Option<String>,
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

pub fn valid_id(id: &str) -> bool {
    id.len() >= 9 && id.len() <= 12 && id.starts_with("tt") && id[2..].bytes().all(|b| b.is_ascii_digit())
}

/// Разбор `chartTitles`: только фильмы и сериалы (без эпизодов и игр).
pub fn parse_chart(v: &Value) -> Vec<Ext> {
    v["edges"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let n = &e["node"];
            let id = text(&n["id"]).filter(|i| valid_id(i))?;
            let t = n["titleType"]["id"].as_str().unwrap_or("movie");
            if matches!(t, "tvEpisode" | "videoGame" | "podcastSeries" | "podcastEpisode") {
                return None;
            }
            let series = n["titleType"]["isSeries"].as_bool().unwrap_or(t.starts_with("tv"));
            let title = text(&n["titleText"]["text"])?;
            let original = text(&n["originalTitleText"]["text"]).filter(|o| *o != title);
            Some(Ext {
                id,
                series,
                title,
                original,
                year: n["releaseYear"]["year"].as_i64().map(|y| y as i32),
                rating: n["ratingsSummary"]["aggregateRating"].as_f64(),
                votes: n["ratingsSummary"]["voteCount"].as_i64(),
                genres: n["genres"]["genres"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|g| text(&g["text"]))
                    .take(3)
                    .collect(),
                poster_url: text(&n["primaryImage"]["url"]).map(|u| sized(&u, 342)),
            })
        })
        .collect()
}

pub fn upsert(app: &App, items: &[Ext]) -> rusqlite::Result<()> {
    let now = now_ms();
    app.db.tx(|tx| {
        for e in items {
            tx.execute(
                "INSERT INTO ext_titles (id, kind, title, original, year, rating, votes, genres, poster_url, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET kind = ?2, title = ?3, original = COALESCE(?4, original),
                   year = COALESCE(?5, year), rating = COALESCE(?6, rating), votes = COALESCE(?7, votes),
                   genres = CASE WHEN ?8 = '[]' THEN genres ELSE ?8 END,
                   poster = CASE WHEN poster_url IS NOT ?9 AND ?9 IS NOT NULL THEN NULL ELSE poster END,
                   poster_url = COALESCE(?9, poster_url), updated_at = ?10",
                params![
                    e.id,
                    if e.series { "series" } else { "movie" },
                    e.title,
                    e.original,
                    e.year,
                    e.rating,
                    e.votes,
                    serde_json::to_string(&e.genres).unwrap_or_else(|_| "[]".into()),
                    e.poster_url,
                    now
                ],
            )?;
        }
        Ok(())
    })
}

fn set_chart(app: &App, key: &str, items: &[Ext]) -> rusqlite::Result<()> {
    app.db.tx(|tx| {
        tx.execute("DELETE FROM ext_charts WHERE chart = ?", [key])?;
        for (i, e) in items.iter().enumerate() {
            tx.execute("INSERT INTO ext_charts (chart, pos, ext_id) VALUES (?, ?, ?)", params![key, i as i64, e.id])?;
        }
        Ok(())
    })
}

/// Обновить подборки с IMDb. Ошибка одной подборки не мешает остальным.
pub async fn refresh(app: &Arc<App>) -> anyhow::Result<usize> {
    let imdb = Imdb::new(app.http.clone());
    let mut done = 0;
    let mut last = None;
    for (key, chart, _) in CHARTS {
        match imdb.data(CHART, json!({ "chart": chart }), "chartTitles").await {
            Ok(v) => {
                let items = parse_chart(&v);
                if !items.is_empty() {
                    upsert(app, &items)?;
                    set_chart(app, key, &items)?;
                    done += 1;
                }
            }
            Err(e) => last = Some(e),
        }
    }
    if done > 0 {
        app.db.set_setting("discover.at", &now_ms().to_string())?;
        app.posters_kick.notify_one();
        Ok(done)
    } else {
        Err(last.unwrap_or_else(|| anyhow::anyhow!("IMDb ничего не вернул")))
    }
}

/// Подборки — раз в 12 часов (первый раз — через минуту после запуска).
pub async fn worker(app: Arc<App>) {
    tokio::time::sleep(Duration::from_secs(60)).await;
    loop {
        let at: i64 = app.db.setting_or("discover.at", "0").parse().unwrap_or(0);
        if now_ms() - at > EVERY.as_millis() as i64 {
            if let Err(e) = refresh(&app).await {
                tracing::warn!(error = %e, "лента IMDb");
            }
        }
        let _ = tokio::time::timeout(Duration::from_secs(3600), app.discover_kick.notified()).await;
    }
}

/// Постеры ленты — по 6 за раз, пока есть нескачанные; затем ждём новых.
pub async fn posters(app: Arc<App>) {
    loop {
        let n = posters_batch(&app, 6).await;
        if n == 0 {
            let _ = tokio::time::timeout(Duration::from_secs(3600), app.posters_kick.notified()).await;
        } else {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }
}

/// Скачать до `limit` постеров (сначала — из подборок по порядку). Сколько пробовали.
pub async fn posters_batch(app: &Arc<App>, limit: i64) -> usize {
    let todo: Vec<(String, String)> = app
        .db
        .with(|c| {
            let mut st = c.prepare(
                "SELECT e.id, e.poster_url FROM ext_titles e WHERE e.poster IS NULL AND e.poster_url IS NOT NULL
                 ORDER BY (SELECT MIN(pos) FROM ext_charts ch WHERE ch.ext_id = e.id) IS NULL,
                          (SELECT MIN(pos) FROM ext_charts ch WHERE ch.ext_id = e.id), e.updated_at DESC
                 LIMIT ?",
            )?;
            let r = st.query_map([limit], |r| Ok((r.get(0)?, r.get(1)?)))?;
            r.collect()
        })
        .unwrap_or_default();
    fetch_posters(app, todo).await
}

/// Постеры именно этих фильмов (ещё не скачанные).
pub async fn posters_of(app: &Arc<App>, ids: &[String]) -> usize {
    let list = serde_json::to_string(ids).unwrap_or_else(|_| "[]".into());
    let todo: Vec<(String, String)> = app
        .db
        .with(|c| {
            let mut st = c.prepare(
                "SELECT id, poster_url FROM ext_titles WHERE poster IS NULL AND poster_url IS NOT NULL
                   AND id IN (SELECT value FROM json_each(?))",
            )?;
            let r = st.query_map([list], |r| Ok((r.get(0)?, r.get(1)?)))?;
            r.collect()
        })
        .unwrap_or_default();
    fetch_posters(app, todo).await
}

async fn fetch_posters(app: &Arc<App>, todo: Vec<(String, String)>) -> usize {
    let jobs = todo.iter().map(|(id, url)| async move {
        // Не вышло — помечаем пустой строкой, чтобы не пробовать по кругу.
        let name = app.images.download(url).await.unwrap_or_default();
        let _ = app.db.with(|c| c.execute("UPDATE ext_titles SET poster = ? WHERE id = ?", params![name, id]));
    });
    futures_util::future::join_all(jobs).await;
    todo.len()
}

/// Картинка к себе (не дольше 8 с); не вышло — без неё.
async fn grab(app: &App, url: Option<&str>) -> Option<String> {
    let url = url?;
    tokio::time::timeout(Duration::from_secs(8), app.images.download(url)).await.ok()?.ok()
}

/// Всё для страницы фильма из ленты — один раз: описание с IMDb, большой постер, кадр, актёры с
/// фото, кадры и похожие (с постерами). Картинки качаются вместе, браузер получит их у нас.
pub async fn detail(app: &Arc<App>, id: &str) -> anyhow::Result<()> {
    let imdb = Imdb::new(app.http.clone());
    let d = imdb.details(id, &[]).await?;
    let similar = match imdb.data(SIMILAR, json!({ "id": id }), "title").await {
        Ok(v) => parse_chart(&v["moreLikeThisTitles"]),
        Err(_) => vec![],
    };
    let e = Ext {
        id: id.to_string(),
        series: d.series,
        title: d.title.clone(),
        original: d.original.clone().filter(|o| *o != d.title),
        year: d.year,
        rating: d.rating,
        votes: d.votes,
        genres: d.genres.clone(),
        poster_url: d.poster.as_deref().map(|u| sized(u, 342)),
    };
    let similar: Vec<Ext> = similar.into_iter().filter(|s| s.id != id).collect();
    upsert(app, &similar)?;
    upsert(app, std::slice::from_ref(&e))?;

    let people: Vec<_> = d.people.iter().take(16).collect();
    let (poster, backdrop, gallery, photos, _) = tokio::join!(
        grab(app, d.poster.as_deref()),
        grab(app, d.backdrop.as_deref()),
        futures_util::future::join_all(d.gallery.iter().take(6).map(|u| grab(app, Some(u)))),
        futures_util::future::join_all(people.iter().map(|p| grab(app, p.photo.as_deref()))),
        async {
            let ids: Vec<String> = similar.iter().map(|s| s.id.clone()).chain([e.id.clone()]).collect();
            posters_of(app, &ids).await
        },
    );
    let extra = json!({
        "tagline": d.tagline,
        "age": d.age,
        "countries": d.countries,
        "languages": d.languages,
        "releaseDate": d.release_date,
        "aka": d.aka,
        "poster": poster,
        "people": people.iter().zip(photos).map(|(p, photo)| json!({
            "name": p.name, "job": p.job, "role": p.role, "photo": photo,
        })).collect::<Vec<_>>(),
        "gallery": gallery.into_iter().flatten().collect::<Vec<_>>(),
        "similar": similar.iter().map(|s| &s.id).collect::<Vec<_>>(),
    });
    app.db.with(|c| {
        c.execute(
            "UPDATE ext_titles SET overview = ?, runtime = ?, backdrop = ?, extra = ?, detailed = ? WHERE id = ?",
            params![d.overview, d.runtime, backdrop, extra.to_string(), DETAILED, id],
        )
    })?;
    Ok(())
}

/// Карточка фильма из ленты для интерфейса.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtCard {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub original: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub poster: Option<String>,
    pub genres: Vec<String>,
    /// Есть на сервисе: id фильма у нас.
    pub local: Option<i64>,
    /// «Буду смотреть» / «Просмотрел».
    pub mark: Option<String>,
    /// Просили хоста добавить.
    pub wished: bool,
}

pub fn card_sql(kids: bool) -> String {
    format!(
        "SELECT e.id, e.kind, e.title, e.original, e.year, e.rating, NULLIF(e.poster, ''), e.genres,
           (SELECT t.id FROM titles t WHERE t.imdb_id = e.id AND {} LIMIT 1),
           (SELECT m.status FROM ext_marks m WHERE m.ext_id = e.id AND m.profile_id = ?1),
           EXISTS (SELECT 1 FROM wishes w WHERE w.ext_id = e.id AND w.profile_id = ?1 AND w.dismissed_at IS NULL)
         FROM ext_titles e",
        crate::api::visible(kids)
    )
}

pub fn card(r: &rusqlite::Row) -> rusqlite::Result<ExtCard> {
    Ok(ExtCard {
        id: r.get(0)?,
        kind: r.get(1)?,
        title: r.get(2)?,
        original: r.get(3)?,
        year: r.get(4)?,
        rating: r.get(5)?,
        poster: crate::api::img(r.get(6)?),
        genres: crate::api::json_list(r.get(7)?),
        local: r.get(8)?,
        mark: r.get(9)?,
        wished: r.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_charts() {
        let v = json!({"edges": [
            {"currentRank": 1, "node": {"id": "tt0111161", "titleText": {"text": "Побег из Шоушенка"},
              "originalTitleText": {"text": "The Shawshank Redemption"}, "titleType": {"id": "movie", "isSeries": false},
              "releaseYear": {"year": 1994}, "ratingsSummary": {"aggregateRating": 9.3, "voteCount": 3000000},
              "primaryImage": {"url": "https://m.media-amazon.com/images/M/x._V1_.jpg"},
              "genres": {"genres": [{"text": "Drama"}]}}},
            {"currentRank": 2, "node": {"id": "tt0903747", "titleText": {"text": "Breaking Bad"},
              "originalTitleText": {"text": "Breaking Bad"}, "titleType": {"id": "tvSeries", "isSeries": true},
              "releaseYear": {"year": 2008}}},
            {"node": {"id": "tt1", "titleText": {"text": "плохой id"}}},
            {"node": {"id": "tt1234567", "titleText": {"text": "Серия"}, "titleType": {"id": "tvEpisode"}}}
        ]});
        let c = parse_chart(&v);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].original.as_deref(), Some("The Shawshank Redemption"));
        assert!(c[0].poster_url.as_deref().unwrap().contains("UX342"));
        assert_eq!((c[1].series, c[1].original.as_deref()), (true, None));
        assert!(valid_id("tt0111161") && !valid_id("tt12") && !valid_id("../etc"));
    }
}
