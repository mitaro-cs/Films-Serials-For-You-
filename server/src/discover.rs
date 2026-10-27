//! Лента «как на IMDb»: подборки (популярное, топ-250) и поиск по всему IMDb - фильмы и сериалы,
//! которых может не быть на компьютере хоста. Их можно отметить «Буду смотреть»/«Просмотрел»,
//! положить в свой список и попросить хоста добавить. Картинки сервер скачивает к себе: браузер
//! ходит только к нам. Подборки обновляются раз в 12 часов, постеры докачиваются в фоне.

use crate::meta::imdb::{sized, Imdb};
use crate::util::now_ms;
use crate::App;
use futures_util::StreamExt;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

/// Откуда подборка: готовый список IMDb, поиск IMDb по жанру и стране или «скоро в кино».
pub enum Src {
    Chart(&'static str),
    /// Типы, жанр IMDb, страны, только вышедшее за последние дни, не меньше стольких оценок.
    Search {
        types: &'static [&'static str],
        genre: Option<&'static str>,
        countries: &'static [&'static str],
        days: Option<i64>,
        votes: i64,
    },
    Soon,
}

pub const MOVIE: &[&str] = &["movie"];
pub const SERIES: &[&str] = &["tvSeries", "tvMiniSeries"];
pub const ANY: &[&str] = &["movie", "tvSeries", "tvMiniSeries"];

/// Запрос - это жанр («фантастика», «комедии», «сериалы ужасы»): жанр IMDb, подпись и что искать
/// (фильмы, сериалы или всё). Слова вроде «фильмы», «лучшие», «про» не мешают; любое другое
/// слово - значит, ищут название.
pub fn genre_query(query: &str) -> Option<(&'static str, &'static str, &'static [&'static str])> {
    const STEMS: &[(&str, &str, &str)] = &[
        ("фантаст", "Sci-Fi", "Фантастика"),
        ("научн", "Sci-Fi", "Фантастика"),
        ("комеди", "Comedy", "Комедия"),
        ("комедийн", "Comedy", "Комедия"),
        ("ужас", "Horror", "Ужасы"),
        ("хоррор", "Horror", "Ужасы"),
        ("боевик", "Action", "Боевик"),
        ("экшн", "Action", "Боевик"),
        ("драм", "Drama", "Драма"),
        ("триллер", "Thriller", "Триллер"),
        ("детектив", "Mystery", "Детектив"),
        ("мелодрам", "Romance", "Мелодрама"),
        ("романтич", "Romance", "Мелодрама"),
        ("приключен", "Adventure", "Приключения"),
        ("фэнтези", "Fantasy", "Фэнтези"),
        ("фентези", "Fantasy", "Фэнтези"),
        ("мульт", "Animation", "Мультфильм"),
        ("анимац", "Animation", "Мультфильм"),
        ("документальн", "Documentary", "Документальный"),
        ("криминал", "Crime", "Криминал"),
        ("военн", "War", "Военный"),
        ("войн", "War", "Военный"),
        ("вестерн", "Western", "Вестерн"),
        ("биограф", "Biography", "Биография"),
        ("историческ", "History", "История"),
        ("мюзикл", "Musical", "Мюзикл"),
        ("спортивн", "Sport", "Спорт"),
        ("семейн", "Family", "Семейный"),
        ("нуар", "Film-Noir", "Нуар"),
    ];
    const FILLER: &[&str] =
        &["лучш", "топ", "про", "кино", "новы", "популярн", "интересн", "хорош", "смотреть", "жанр"];
    let q = crate::util::norm(query);
    let mut genre = None;
    let mut types: &'static [&'static str] = ANY;
    for w in q.split_whitespace() {
        if w.starts_with("сериал") {
            types = SERIES;
        } else if w.starts_with("фильм") && !w.starts_with("фильмограф") {
            types = MOVIE;
        } else if let Some(&(_, en, ru)) = STEMS.iter().find(|(s, ..)| w.starts_with(s)) {
            // «мультсериалы» - мультфильм и сериал.
            if w.contains("сериал") {
                types = SERIES;
            }
            if genre.is_some_and(|(g, _)| g != en) {
                return None;
            }
            genre = Some((en, ru));
        } else if !FILLER.iter().any(|f| w.starts_with(f)) {
            return None;
        }
    }
    genre.map(|(en, ru)| (en, ru, types))
}

/// Популярное в жанре с IMDb (до 50, сначала самое известное): ids в ленте, на 12 часов.
pub async fn genre_search(
    app: &Arc<App>,
    genre: &'static str,
    types: &'static [&'static str],
) -> anyhow::Result<Vec<String>> {
    let src = Src::Search { types, genre: Some(genre), countries: &[], days: None, votes: 25_000 };
    search_cached(app, &format!("genre:{genre}:{}:votes", types.len()), &src).await
}

/// Поиск IMDb по источнику (жанр, страны), больше всего оценок - первым; ids в ленте, на 12 часов.
pub async fn search_cached(app: &Arc<App>, key: &str, src: &Src) -> anyhow::Result<Vec<String>> {
    let cached: Option<String> = app.db.with(|c| {
        c.query_row(
            "SELECT ids FROM catalog_search_cache WHERE query = ? AND at > ?",
            params![key, now_ms() - 12 * 3_600_000],
            |r| r.get(0),
        )
        .optional()
    })?;
    if let Some(c) = cached {
        return Ok(serde_json::from_str(&c)?);
    }
    let imdb = Imdb::new(app.http.clone());
    let items = parse_chart(&fetch_feed(&imdb, src, "USER_RATING_COUNT").await?);
    upsert(app, &items)?;
    app.posters_kick.notify_one();
    let ids: Vec<String> = items.into_iter().map(|e| e.id).collect();
    if !ids.is_empty() {
        app.db.with(|c| {
            c.execute(
                "INSERT INTO catalog_search_cache VALUES (?, ?, ?)
                 ON CONFLICT(query) DO UPDATE SET ids = excluded.ids, at = excluded.at",
                params![key, json!(ids).to_string(), now_ms()],
            )
        })?;
    }
    Ok(ids)
}

const fn genre(types: &'static [&'static str], g: &'static str, votes: i64) -> Src {
    Src::Search { types, genre: Some(g), countries: &[], days: None, votes }
}

/// Подборки: ключ у нас (вкладки «Фильмы»/«Сериалы» берут `*_movies`/`*_series`), подпись, откуда.
pub const FEEDS: &[(&str, &str, Src)] = &[
    ("popular_movies", "Популярные фильмы", Src::Chart("MOST_POPULAR_MOVIES")),
    ("popular_series", "Популярные сериалы", Src::Chart("MOST_POPULAR_TV_SHOWS")),
    (
        "new_movies",
        "Новинки кино",
        Src::Search { types: MOVIE, genre: None, countries: &[], days: Some(180), votes: 1000 },
    ),
    (
        "new_series",
        "Новые сериалы",
        Src::Search { types: SERIES, genre: None, countries: &[], days: Some(180), votes: 1000 },
    ),
    ("soon_movies", "Скоро в кино", Src::Soon),
    ("top_movies", "Топ-250 фильмов", Src::Chart("TOP_RATED_MOVIES")),
    ("top_series", "Лучшие сериалы", Src::Chart("TOP_RATED_TV_SHOWS")),
    (
        "russian_movies",
        "Российское кино",
        Src::Search { types: MOVIE, genre: None, countries: &["RU", "SUHH"], days: None, votes: 3000 },
    ),
    (
        "russian_series",
        "Российские сериалы",
        Src::Search { types: SERIES, genre: None, countries: &["RU"], days: None, votes: 1000 },
    ),
    ("comedy_movies", "Комедии", genre(MOVIE, "Comedy", 20000)),
    ("action_movies", "Боевики", genre(MOVIE, "Action", 20000)),
    ("thriller_movies", "Триллеры", genre(MOVIE, "Thriller", 20000)),
    ("horror_movies", "Ужасы", genre(MOVIE, "Horror", 15000)),
    ("scifi_movies", "Фантастика", genre(MOVIE, "Sci-Fi", 20000)),
    ("drama_movies", "Драмы", genre(MOVIE, "Drama", 20000)),
    ("romance_movies", "Мелодрамы", genre(MOVIE, "Romance", 15000)),
    ("animation_movies", "Мультфильмы", genre(MOVIE, "Animation", 10000)),
    ("family_movies", "Для всей семьи", genre(MOVIE, "Family", 10000)),
    ("documentary_movies", "Документальное", genre(MOVIE, "Documentary", 3000)),
    ("crime_series", "Криминальные сериалы", genre(SERIES, "Crime", 10000)),
    ("comedy_series", "Комедийные сериалы", genre(SERIES, "Comedy", 10000)),
    ("scifi_series", "Фантастические сериалы", genre(SERIES, "Sci-Fi", 10000)),
    (
        "anime_movies",
        "Аниме-фильмы",
        Src::Search { types: MOVIE, genre: Some("Animation"), countries: &["JP"], days: None, votes: 1000 },
    ),
    (
        "anime_series",
        "Аниме-сериалы",
        Src::Search { types: SERIES, genre: Some("Animation"), countries: &["JP"], days: None, votes: 5000 },
    ),
];

const NODE: &str = "id titleText { text } originalTitleText { text } titleType { id isSeries }
    releaseYear { year } ratingsSummary { aggregateRating voteCount }
    primaryImage { url } genres { genres { text } }";

/// Поиск IMDb: `POPULARITY` - сначала модное сейчас, `USER_RATING_COUNT` - больше всего оценок (классика жанра).
fn search_query(sort: &str) -> String {
    let order = if sort == "POPULARITY" { "ASC" } else { "DESC" };
    format!(
        "query A($c: AdvancedTitleSearchConstraints!) {{ advancedTitleSearch(first: 50, constraints: $c,
           sort: {{ sortBy: {sort}, sortOrder: {order} }}) {{ edges {{ node {{ title {{ {NODE} }} }} }} }} }}"
    )
}

fn soon_query() -> String {
    format!(
        "query S($from: Date!) {{ comingSoon(first: 50, comingSoonType: MOVIE, releasingOnOrAfter: $from) {{
           edges {{ node {{ {NODE} }} }} }} }}"
    )
}

/// Дата `YYYY-MM-DD` по миллисекундам Unix (UTC).
pub fn ymd(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    // Алгоритм Хиннанта: дни от 1970-01-01 → гражданская дата.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Подборка с IMDb в виде `chartTitles` (`edges[].node` - фильм).
async fn fetch_feed(imdb: &Imdb, src: &Src, sort: &str) -> anyhow::Result<Value> {
    let now = now_ms();
    match src {
        Src::Chart(chart) => imdb.data(CHART, json!({ "chart": chart }), "chartTitles").await,
        Src::Soon => imdb.data(&soon_query(), json!({ "from": ymd(now) }), "comingSoon").await,
        Src::Search { types, genre, countries, days, votes } => {
            let mut c = json!({
                "titleTypeConstraint": { "anyTitleTypeIds": types },
                "userRatingsConstraint": { "ratingsCountRange": { "min": votes } },
            });
            if let Some(g) = genre {
                c["genreConstraint"] = json!({ "allGenreIds": [g] });
            }
            if !countries.is_empty() {
                c["originCountryConstraint"] = json!({ "anyPrimaryCountries": countries });
            }
            if let Some(d) = days {
                c["releaseDateConstraint"] =
                    json!({ "releaseDateRange": { "start": ymd(now - d * 86_400_000), "end": ymd(now) } });
            }
            let v = imdb.data(&search_query(sort), json!({ "c": c }), "advancedTitleSearch").await?;
            let edges: Vec<Value> =
                v["edges"].as_array().into_iter().flatten().map(|e| json!({ "node": e["node"]["title"] })).collect();
            Ok(json!({ "edges": edges }))
        }
    }
}

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

/// Описание фильма из ленты в нынешнем виде (`ext_titles.detailed`): 1 - только сюжет (0.6),
/// 2 - со своей страницей: кадр, актёры, кадры, похожие; 3 (1.0) - и сезоны сериала (второй
/// запрос IMDb снова работает: актёры, кадры и русские названия - заново).
/// 5 (alpha 0.4) - кадр баннера из фильма (`still_frame` около 16:9), а не рекламный плакат.
pub const DETAILED: i64 = 5;

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
                   poster = CASE WHEN COALESCE(poster_url, '') NOT LIKE '%tmdb.org/%' AND poster_url IS NOT ?9 AND ?9 IS NOT NULL
                     THEN NULL ELSE poster END,
                   poster_url = CASE WHEN poster_url LIKE '%tmdb.org/%' THEN poster_url ELSE COALESCE(?9, poster_url) END,
                   updated_at = ?10",
                params![
                    e.id,
                    if e.series { "series" } else { "movie" },
                    e.title,
                    e.original,
                    e.year,
                    e.rating,
                    e.votes,
                    serde_json::to_string(&crate::meta::genres_ru(&e.genres)).unwrap_or_else(|_| "[]".into()),
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
    for (key, _, src) in FEEDS {
        match tokio::time::timeout(Duration::from_secs(8), fetch_feed(&imdb, src, "POPULARITY"))
            .await
            .unwrap_or_else(|e| Err(e.into()))
        {
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
        // IMDb не любит залпы: подборки - по очереди с паузой.
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    if done > 0 {
        app.db.set_setting("discover.at", &now_ms().to_string())?;
        app.db.set_setting("discover.feeds", &FEEDS.len().to_string())?;
        app.posters_kick.notify_one();
        Ok(done)
    } else {
        Err(last.unwrap_or_else(|| anyhow::anyhow!("IMDb ничего не вернул")))
    }
}

/// Подборки - раз в 12 часов (первый раз - через минуту после запуска).
pub async fn worker(app: Arc<App>) {
    tokio::time::sleep(Duration::from_secs(2)).await;
    loop {
        let at: i64 = app.db.setting_or("discover.at", "0").parse().unwrap_or(0);
        // Новых подборок ещё нет (обновили программу) - сразу, не дожидаясь 12 часов.
        let fresh_set = app.db.setting_or("discover.feeds", "0") == FEEDS.len().to_string();
        if now_ms() - at > EVERY.as_millis() as i64 || !fresh_set {
            if let Err(e) = refresh(&app).await {
                tracing::warn!(error = %e, "лента IMDb");
            }
        }
        let _ = tokio::time::timeout(Duration::from_secs(3600), app.discover_kick.notified()).await;
    }
}

/// Постеры ленты - по 6 за раз, пока есть нескачанные; затем ждём новых.
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

/// Скачать до `limit` постеров (сначала - из подборок по порядку). Сколько пробовали.
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

/// Постер больше этого (байт) - уменьшаем до ширины карточки: запасные источники (TVmaze, Википедия)
/// отдают оригиналы по 2-3 МБ, а показываем мы 150-340 px.
const POSTER_MAX_BYTES: u64 = 200_000;

/// Постер для карточки: большой - уменьшить до 342 px (нет ffmpeg - оставить как есть).
pub async fn small_poster(app: &App, name: String) -> String {
    let Some(path) = app.images.path(&name).filter(|p| p.metadata().is_ok_and(|m| m.len() > POSTER_MAX_BYTES)) else {
        return name;
    };
    let ffmpeg = app.tools.read().ffmpeg.clone();
    let Some(ffmpeg) = ffmpeg else { return name };
    match crate::library::probe::scale_image(&ffmpeg, &path, 342).await.and_then(|b| app.images.save(&b)) {
        Ok(small) => small,
        Err(_) => name,
    }
}

/// Один раз после запуска: уже скачанные большие постеры ленты - уменьшить (понемногу, в фоне).
pub async fn shrink_posters(app: Arc<App>) {
    tokio::time::sleep(Duration::from_secs(60)).await;
    let all: Vec<(String, String)> = app
        .db
        .with(|c| {
            let mut st = c.prepare("SELECT id, poster FROM ext_titles WHERE poster IS NOT NULL AND poster != ''")?;
            let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            r.collect()
        })
        .unwrap_or_default();
    for (id, name) in all {
        let small = small_poster(&app, name.clone()).await;
        if small != name {
            let _ = app.db.with(|c| c.execute("UPDATE ext_titles SET poster = ? WHERE id = ?", params![small, id]));
        }
    }
}

async fn fetch_posters(app: &Arc<App>, todo: Vec<(String, String)>) -> usize {
    let jobs = todo.iter().map(|(id, url)| async move {
        // Не вышло - помечаем пустой строкой, чтобы не пробовать по кругу.
        let name = app.images.download(url).await.unwrap_or_default();
        let name = if name.is_empty() { name } else { small_poster(app, name).await };
        let _ = app.db.with(|c| c.execute("UPDATE ext_titles SET poster = ? WHERE id = ?", params![name, id]));
    });
    futures_util::future::join_all(jobs).await;
    todo.len()
}

/// Что сейчас качается: два открытия одного фильма не идут к IMDb дважды.
static LOADING: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(Default::default);

/// Всё для страницы фильма из ленты - один раз: описание с IMDb, большой постер, кадр, актёры с
/// фото, кадры и похожие (с постерами). Картинки качаются вместе, браузер получит их у нас.
/// Что-то не вышло - сохраняем что есть (прежнее не теряем), `detailed` = 1 и время попытки:
/// повторим через `RETRY_MS`. Уже качается — `Ok(false)`.
pub async fn detail(app: &Arc<App>, id: &str) -> anyhow::Result<bool> {
    if !LOADING.lock().unwrap().insert(id.to_string()) {
        return Ok(false);
    }
    struct Done<'a>(&'a str);
    impl Drop for Done<'_> {
        fn drop(&mut self) {
            LOADING.lock().unwrap().remove(self.0);
        }
    }
    let _done = Done(id);
    let res = fetch_detail(app, id).await;
    if res.is_err() {
        let _ = app.db.with(|c| {
            c.execute(
                "UPDATE ext_titles SET extra = json_set(COALESCE(extra, '{}'), '$.at', ?) WHERE id = ?",
                params![now_ms(), id],
            )
        });
    }
    res.map(|()| true)
}

async fn fetch_detail(app: &Arc<App>, id: &str) -> anyhow::Result<()> {
    let imdb = Imdb::new(app.http.clone());
    let d = imdb.details(id, &[]).await?;
    let e = Ext {
        id: id.into(),
        series: d.series,
        title: d.title.clone(),
        original: d.original.clone(),
        year: d.year,
        rating: d.rating,
        votes: d.votes,
        genres: d.genres.clone(),
        poster_url: d.poster.as_deref().map(|u| sized(u, 342)),
    };
    upsert(app, &[e])?;
    // Есть ключ TMDB - русский постер вместо промо-плаката IMDb (upsert его потом не затирает).
    if let Some(tmdb) = crate::meta::tmdb_posters(app) {
        if let Ok(Ok(Some(url))) = tokio::time::timeout(Duration::from_secs(4), tmdb.poster_ru(id)).await {
            app.db.with(|c| {
                c.execute(
                    "UPDATE ext_titles SET poster_url = ?1, poster = NULL WHERE id = ?2 AND poster_url IS NOT ?1",
                    params![url, id],
                )
            })?;
            app.posters_kick.notify_one();
        }
    }
    let mut extra = json!({"tagline":d.tagline,"age":d.age,"countries":d.countries,"languages":d.languages,"releaseDate":d.release_date,"aka":d.aka,
      "poster":d.poster.as_deref().map(crate::media_meta::art),"people":d.people.iter().take(16).map(|p|json!({"name":p.name,"job":p.job,"role":p.role,"photo":p.photo.as_deref().map(crate::media_meta::art),"id":p.id})).collect::<Vec<_>>(),
      "gallery":d.gallery.iter().take(6).map(|u|crate::media_meta::art(u)).collect::<Vec<_>>(),"similar":[],"seasons":d.seasons});
    if extra["seasons"].as_array().is_some_and(|s| s.is_empty()) {
        extra.as_object_mut().expect("extra object").remove("seasons");
    }
    app.db.with(|c|c.execute("UPDATE ext_titles SET overview=COALESCE(overview,?1),runtime=?2,backdrop=COALESCE(?3,NULLIF(backdrop,'')),extra=json_patch(COALESCE(extra,'{}'),?4),detailed=?5 WHERE id=?6",params![d.overview,d.runtime,d.backdrop.as_deref().map(crate::media_meta::art),extra.to_string(),DETAILED,id]))?;
    let a = app.clone();
    let id = id.to_string();
    let rich_job = format!("rich:{id}");
    app.detail_jobs.lock().insert(rich_job.clone());
    tokio::spawn(async move {
        let provider = Imdb::new(a.http.clone());
        let (ru, related) = tokio::join!(
            crate::ruwiki::overview(&a.http, &id),
            tokio::time::timeout(Duration::from_secs(8), provider.data(SIMILAR, json!({"id":id}), "title"))
        );
        if let Some(ru) = ru {
            let _ = a.db.with(|c| c.execute("UPDATE ext_titles SET overview=? WHERE id=?", params![ru, id]));
        }
        if let Ok(Ok(v)) = related {
            let items = parse_chart(&v["moreLikeThisTitles"]).into_iter().filter(|e| e.id != id).collect::<Vec<_>>();
            let _ = upsert(&a, &items);
            let _ = a.db.with(|c| {
                c.execute(
                    "UPDATE ext_titles SET extra=json_set(extra,'$.similar',json(?)) WHERE id=?",
                    params![json!(items.iter().map(|e| &e.id).collect::<Vec<_>>()).to_string(), id],
                )
            });
            a.posters_kick.notify_one();
        }
        a.detail_jobs.lock().remove(&rich_job);
    });
    Ok(())
}

/// Сезон сериала из ленты уже есть и свежий: старые сезоны - навсегда, проверяем раз в неделю.
pub fn season_fresh(app: &App, id: &str, season: i32) -> bool {
    let at: Option<i64> = app
        .db
        .with(|c| {
            c.query_row("SELECT at FROM ext_seasons WHERE ext_id = ? AND season = ?", params![id, season], |r| r.get(0))
                .optional()
        })
        .ok()
        .flatten();
    at.is_some_and(|at| now_ms() - at < 7 * 86_400_000)
}

/// Серии одного сезона сериала из ленты - с IMDb (один раз в неделю), кадры - к себе.
pub async fn season(app: &Arc<App>, id: &str, season: i32) -> anyhow::Result<()> {
    season_data(app, id, season).await?;
    stills(app, id, season).await;
    Ok(())
}

/// Только серии сезона (названия, оценки, даты) - без кадров (сетке оценок кадры не нужны).
/// true - сезон обновился.
pub async fn season_data(app: &Arc<App>, id: &str, season: i32) -> anyhow::Result<bool> {
    if season_fresh(app, id, season) {
        return Ok(false);
    }
    let imdb = Imdb::new(app.http.clone());
    let v = imdb.data(crate::meta::imdb::EPISODES, json!({ "id": id, "season": season.to_string() }), "title").await?;
    let eps = crate::meta::imdb::parse_episodes(&v);
    app.db.tx(|tx| {
        for e in &eps {
            tx.execute(
                "INSERT INTO ext_episodes (ext_id, season, episode, name, overview, still_url, air_date, runtime, rating)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(ext_id, season, episode) DO UPDATE SET name = ?4, overview = ?5,
                   still = CASE WHEN still_url IS NOT ?6 THEN NULL ELSE still END, still_url = ?6,
                   air_date = ?7, runtime = ?8, rating = ?9",
                params![id, e.season, e.episode, e.name, e.overview, e.still, e.air_date, e.runtime, e.rating],
            )?;
        }
        tx.execute(
            "INSERT INTO ext_seasons (ext_id, season, at) VALUES (?, ?, ?)
             ON CONFLICT(ext_id, season) DO UPDATE SET at = excluded.at",
            params![id, season, now_ms()],
        )?;
        Ok(())
    })?;
    Ok(true)
}

/// Кадры серий сезона, которых ещё нет у нас (по 6 разом, не дольше 12 с; остальные - в следующий раз).
pub async fn stills(app: &Arc<App>, id: &str, season: i32) {
    let todo: Vec<(i32, String)> = app
        .db
        .with(|c| {
            let mut st = c.prepare(
                "SELECT episode, still_url FROM ext_episodes
                 WHERE ext_id = ? AND season = ? AND (still IS NULL OR still = '') AND still_url IS NOT NULL ORDER BY episode",
            )?;
            let r = st.query_map(params![id, season], |r| Ok((r.get(0)?, r.get(1)?)))?;
            r.collect()
        })
        .unwrap_or_default();
    let work = futures_util::stream::iter(todo)
        .map(|(ep, url)| async move {
            let Ok(name) = app.images.download(&url).await else { return };
            let _ = app.db.with(|c| {
                c.execute(
                    "UPDATE ext_episodes SET still = ? WHERE ext_id = ? AND season = ? AND episode = ?",
                    params![name, id, season, ep],
                )
            });
        })
        .buffer_unordered(6)
        .collect::<Vec<_>>();
    let _ = tokio::time::timeout(Duration::from_secs(12), work).await;
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
        "SELECT e.id, e.kind, COALESCE((SELECT title FROM title_localizations l WHERE l.id=e.id),e.title),
           COALESCE(e.original,CASE WHEN EXISTS(SELECT 1 FROM title_localizations l WHERE l.id=e.id AND l.title!=e.title) THEN e.title END), e.year, e.rating,
           COALESCE(NULLIF(e.poster, ''),'/api/discover/poster/'||e.id), e.genres,
           (SELECT t.id FROM titles t WHERE t.imdb_id = e.id AND {} LIMIT 1),
           CASE WHEN EXISTS (SELECT 1 FROM ext_dropped d WHERE d.ext_id = e.id AND d.profile_id = ?1) THEN 'dropped'
             ELSE (SELECT m.status FROM ext_marks m WHERE m.ext_id = e.id AND m.profile_id = ?1) END,
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
        poster: r.get::<_, Option<String>>(6)?.and_then(|s| {
            if s.starts_with('/') {
                Some(s)
            } else {
                crate::api::img(Some(s))
            }
        }),
        genres: crate::meta::genres_ru(crate::api::json_list(r.get(7)?)),
        local: r.get(8)?,
        mark: r.get(9)?,
        wished: r.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genre_queries() {
        let g = |q: &str| genre_query(q).map(|(en, _, t)| (en, t.len()));
        assert_eq!(g("Фантастика"), Some(("Sci-Fi", 3)));
        assert_eq!(g("комедии"), Some(("Comedy", 3)));
        assert_eq!(g("сериалы ужасы"), Some(("Horror", 2)));
        assert_eq!(g("лучшие фильмы про войну"), Some(("War", 1)));
        assert_eq!(g("военные фильмы"), Some(("War", 1)));
        assert_eq!(g("мультсериалы"), Some(("Animation", 2)));
        assert_eq!(g("Тёмный рыцарь"), None);
        assert_eq!(g("фантастика драма"), None);
        assert_eq!(g("фильмы"), None);
    }

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

    #[test]
    fn dates_and_feeds() {
        assert_eq!(ymd(0), "1970-01-01");
        assert_eq!(ymd(951_782_400_000), "2000-02-29");
        assert_eq!(ymd(1_792_281_600_000), "2026-10-18");
        // Ключи уникальны и с понятным разделом: вкладки берут `*_movies` и `*_series`.
        let keys: std::collections::HashSet<_> = FEEDS.iter().map(|f| f.0).collect();
        assert_eq!(keys.len(), FEEDS.len());
        assert!(FEEDS.iter().all(|f| f.0.ends_with("_movies") || f.0.ends_with("_series")));
    }
}
