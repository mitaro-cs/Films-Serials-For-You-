//! Лента IMDb для интерфейса: подборки, поиск по всему IMDb, карточка фильма не с сервиса,
//! «Буду смотреть»/«Просмотрел», «Хочу на сервисе» (просьба хосту) и список просьб у хоста.

use super::{cards_by_ids, Card};
use crate::auth::{Manager, Viewer};
use crate::discover::{self, card, card_sql, ExtCard, FEEDS};
use crate::error::{ApiError, ApiResult};
use crate::meta::imdb::Imdb;
use crate::util::now_ms;
use crate::App;
use axum::extract::{Path, Query, State};
use axum::Json;
use futures_util::StreamExt;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

const ROW: i64 = 40;

pub(super) fn ext_cards(
    app: &App,
    v: &Viewer,
    filter: &str,
    order: &str,
    p: &[&dyn rusqlite::ToSql],
) -> ApiResult<Vec<ExtCard>> {
    let sql = format!("{} WHERE {filter} ORDER BY {order}", card_sql(v.profile.kids));
    let mut args: Vec<&dyn rusqlite::ToSql> = vec![&v.profile.id];
    args.extend_from_slice(p);
    Ok(app.db.with(|c| {
        let mut st = c.prepare(&sql)?;
        let r = st.query_map(args.as_slice(), card)?;
        r.collect::<rusqlite::Result<Vec<_>>>()
    })?)
}

pub fn ext_card(app: &App, v: &Viewer, id: &str) -> ApiResult<ExtCard> {
    ext_cards(app, v, "e.id = ?2", "e.id", &[&id])?.pop().ok_or_else(ApiError::not_found)
}

/// Подборки ленты. Пусто (ещё не загружали) - просим обновить.
pub async fn feed(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let mut rows = Vec::new();
    if let Some(row) = because(&app, &v)? {
        rows.push(row);
    }
    for (key, title, _) in FEEDS {
        let mut items = ext_cards(
            &app,
            &v,
            &format!("e.id IN (SELECT ext_id FROM ext_charts WHERE chart = ?2 AND pos < {ROW})"),
            "(SELECT pos FROM ext_charts ch WHERE ch.chart = ?2 AND ch.ext_id = e.id)",
            &[key],
        )?;
        items.retain(|c| c.mark.as_deref() != Some("dropped"));
        if !items.is_empty() {
            rows.push(json!({"key": key, "title": title, "items": items}));
        }
    }
    if rows.is_empty() {
        app.discover_kick.notify_one();
    }
    let at: i64 = app.db.setting_or("discover.at", "0").parse().unwrap_or(0);
    Ok(Json(json!({"rows": rows, "updatedAt": (at > 0).then_some(at)})))
}

/// «Потому что вам понравилось»: похожие на последнее, что человек смотрел, отметил или любит
/// (у фильма должна быть своя страница - там похожие уже есть).
fn because(app: &App, v: &Viewer) -> ApiResult<Option<Value>> {
    let seed: Option<(String, String, String, String)> = app.db.with(|c| {
        c.query_row(
            "SELECT e.id, e.title, e.kind, e.extra FROM ext_titles e
             WHERE e.extra IS NOT NULL AND json_array_length(json_extract(e.extra, '$.similar')) > 3 AND (
               e.id IN (SELECT ext_id FROM ext_marks WHERE profile_id = ?1 AND status IN ('watched', 'watching'))
               OR e.id IN (SELECT t.imdb_id FROM watched w JOIN titles t ON t.id = w.title_id WHERE w.profile_id = ?1)
               OR e.id IN (SELECT substr(j.value, 3) FROM profiles p, json_each(p.favorites) j
                           WHERE p.id = ?1 AND j.value LIKE 'x:%'))
             ORDER BY MAX(COALESCE((SELECT m.at FROM ext_marks m WHERE m.ext_id = e.id AND m.profile_id = ?1), 0),
                          COALESCE((SELECT w.at FROM watched w JOIN titles t ON t.id = w.title_id
                                    WHERE t.imdb_id = e.id AND w.profile_id = ?1), 0)) DESC
             LIMIT 1",
            [v.profile.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
    })?;
    let Some((_, title, kind, extra)) = seed else { return Ok(None) };
    let x: Value = serde_json::from_str(&extra).unwrap_or(Value::Null);
    let items = ext_cards(
        app,
        v,
        "e.id IN (SELECT value FROM json_each(?2))",
        "(SELECT key FROM json_each(?2) j WHERE j.value = e.id)",
        &[&x["similar"].to_string()],
    )?;
    let key = if kind == "series" { "because_series" } else { "because_movies" };
    Ok(Some(json!({"key": key, "title": format!("Потому что вам понравилось «{title}»"), "items": items})))
}

/// Вся подборка целиком (например, топ-250).
pub async fn chart(State(app): State<Arc<App>>, v: Viewer, Path(key): Path<String>) -> ApiResult<Json<Value>> {
    let title = FEEDS.iter().find(|c| c.0 == key).map(|c| c.1).ok_or_else(ApiError::not_found)?;
    let items = ext_cards(
        &app,
        &v,
        "e.id IN (SELECT ext_id FROM ext_charts WHERE chart = ?2)",
        "(SELECT pos FROM ext_charts ch WHERE ch.chart = ?2 AND ch.ext_id = e.id)",
        &[&key],
    )?;
    Ok(Json(json!({"key": key, "title": title, "items": items})))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: String,
    scope: Option<String>,
}

/// Поиск по всему IMDb: найденное сохраняется в ленту, постеры - скачиваются сразу (до 6 с).
pub async fn search(
    State(app): State<Arc<App>>,
    v: Viewer,
    Query(q): Query<SearchQuery>,
) -> ApiResult<axum::response::Response> {
    let query = q.q.trim();
    if query.chars().count() < 2 {
        return Ok(axum::response::IntoResponse::into_response(Json(json!([]))));
    }
    if query.chars().count() > 120 {
        return Err(ApiError::bad("query", "Поисковый запрос слишком длинный"));
    }
    if q.scope.as_deref() == Some("people") {
        let result = crate::catalog_search::wiki_search(&app, query, true)
            .await
            .map_err(|e| ApiError::bad("search", format!("Не удалось найти людей: {e}")))?;
        return Ok(axum::response::IntoResponse::into_response(Json(json!(result))));
    }
    let ids = crate::catalog_search::search(&app, query)
        .await
        .map_err(|e| ApiError::bad("search", format!("Поиск временно недоступен: {e}")))?;
    let mut out = Vec::new();
    for id in &ids {
        if let Ok(c) = ext_card(&app, &v, id) {
            out.push(c);
        }
    }
    let norm = crate::util::norm(query);
    out.sort_by_key(|c| search_rank(&norm, &c.title, c.original.as_deref(), c.rating));
    let mut response = axum::response::IntoResponse::into_response(Json(json!(out)));
    if app.detail_jobs.lock().contains(&format!("search:{}", crate::util::norm(query))) {
        response.headers_mut().insert("x-nova-refreshing", axum::http::HeaderValue::from_static("1"));
    }
    Ok(response)
}

/// Постер загружается отдельным запросом картинки, не задерживая JSON поиска.
pub async fn poster(
    State(app): State<Arc<App>>,
    _v: Viewer,
    Path(id): Path<String>,
) -> ApiResult<axum::response::Redirect> {
    if !discover::valid_id(&id) {
        return Err(ApiError::not_found());
    }
    let (saved, mut url): (Option<String>, Option<String>) = app.db.with(|c| {
        c.query_row("SELECT NULLIF(poster,''),poster_url FROM ext_titles WHERE id=?", [&id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
    })?;
    if let Some(name) = saved.filter(|name| app.images.path(name).is_some_and(|path| path.is_file())) {
        return Ok(axum::response::Redirect::temporary(&format!("/img/{name}")));
    }
    let miss = format!("poster.miss.{id}.{}", crate::util::sha256_hex(url.as_deref().unwrap_or("").as_bytes()));
    if now_ms() - app.db.setting_or(&miss, "0").parse::<i64>().unwrap_or(0) < 600_000 {
        return Err(ApiError::not_found());
    }
    let mut name = None;
    if let Some(image) = &url {
        name = tokio::time::timeout(Duration::from_secs(6), app.images.download(image)).await.ok().and_then(Result::ok);
    }
    if name.is_none() {
        let candidate = tokio::time::timeout(Duration::from_secs(6), crate::media_meta::poster_candidate(&app, &id))
            .await
            .ok()
            .and_then(Result::ok)
            .flatten();
        if let Some(image) = candidate.filter(|candidate| Some(candidate) != url.as_ref()) {
            name = tokio::time::timeout(Duration::from_secs(6), app.images.download(&image))
                .await
                .ok()
                .and_then(Result::ok);
            url = Some(image);
        }
    }
    let Some(name) = name else {
        let _ = app.db.set_setting(&miss, &now_ms().to_string());
        return Err(ApiError::not_found());
    };
    let name = discover::small_poster(&app, name).await;
    app.db.with(|c| c.execute("UPDATE ext_titles SET poster=?,poster_url=? WHERE id=?", params![name, url, id]))?;
    Ok(axum::response::Redirect::temporary(&format!("/img/{name}")))
}

/// Порядок в поиске: точное совпадение названия или оригинального названия - выше, чем «начинается с»
/// и «содержит» («Fight Club» - сначала «Бойцовский клуб», потом «Superhero Fight Club»); при равенстве -
/// выше оценка IMDb.
fn search_rank(
    query: &str,
    title: &str,
    original: Option<&str>,
    rating: Option<f64>,
) -> (std::cmp::Reverse<u8>, std::cmp::Reverse<i64>) {
    let score = |name: &str| {
        let name = crate::util::norm(name);
        if name == query {
            100
        } else if name.starts_with(query) {
            80
        } else if name.contains(query) {
            60
        } else {
            20
        }
    };
    let best = score(title).max(original.map(score).unwrap_or(0));
    (std::cmp::Reverse(best), std::cmp::Reverse((rating.unwrap_or(0.0) * 10.0) as i64))
}

/// Страница фильма из ленты: описание, кадр, актёры, кадры и похожие (с IMDb, один раз -
/// дальше из базы), мои списки, ссылки на видео.
pub async fn title(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    if !discover::valid_id(&id) {
        return Err(ApiError::not_found());
    }
    let mut known: Option<i64> =
        app.db.with(|c| c.query_row("SELECT detailed FROM ext_titles WHERE id = ?", [&id], |r| r.get(0)).optional())?;
    if known.is_none() {
        let got = tokio::time::timeout(Duration::from_secs(7), discover::detail(&app, &id)).await;
        if !matches!(got, Ok(Ok(_))) {
            return Err(ApiError::bad("imdb", "IMDb сейчас не отвечает - попробуйте позже"));
        }
    }
    if known.is_none() {
        known = app
            .db
            .with(|c| c.query_row("SELECT detailed FROM ext_titles WHERE id=?", [&id], |r| r.get(0)).optional())?;
    }
    // Тот же фильм уже качает другое открытие - а записи ещё нет.
    if known.is_none() {
        return Err(ApiError::bad("imdb", "IMDb сейчас не отвечает - попробуйте позже"));
    }
    let retry_key = format!("detail.retry.{id}");
    let retry = app.db.setting_or(&retry_key, "0").parse::<i64>().unwrap_or(0);
    let missing_series_art: bool = app.db.with(|c| {
        c.query_row(
            "SELECT kind='series' AND (backdrop IS NULL OR backdrop='') FROM ext_titles WHERE id=?",
            [&id],
            |r| r.get(0),
        )
    })?;
    let refreshing = (known != Some(discover::DETAILED) || missing_series_art) && now_ms() - retry > 5 * 60_000;
    if refreshing && app.detail_jobs.lock().insert(id.clone()) {
        let a = app.clone();
        let title_id = id.clone();
        tokio::spawn(async move {
            let _ = a.db.set_setting(&retry_key, &now_ms().to_string());
            if missing_series_art {
                let _ = tokio::time::timeout(Duration::from_secs(6), crate::media_meta::series(&a, &title_id)).await;
            }
            let result = tokio::time::timeout(Duration::from_secs(12), discover::detail(&a, &title_id)).await;
            if !matches!(result, Ok(Ok(_))) {
                let _ = tokio::time::timeout(
                    Duration::from_secs(18),
                    crate::catalog_search::fallback_detail(&a, &title_id),
                )
                .await;
            }
            a.detail_jobs.lock().remove(&title_id);
        });
    }
    let card = ext_card(&app, &v, &id)?;
    type Row = (Option<String>, Option<i64>, Option<i64>, Option<String>, Option<String>);
    let (overview, runtime, votes, backdrop, extra): Row = app.db.with(|c| {
        c.query_row("SELECT overview, runtime, votes, backdrop, extra FROM ext_titles WHERE id = ?", [&id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
    })?;
    let x: Value = extra.and_then(|e| serde_json::from_str(&e).ok()).unwrap_or(Value::Null);
    let img = crate::media_meta::existing_image;
    let people: Vec<Value> = x["people"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| json!({"name": p["name"], "job": p["job"], "role": p["role"], "photo": img(&p["photo"]), "id": p["id"]}))
        .collect();
    let gallery: Vec<String> = x["gallery"].as_array().into_iter().flatten().filter_map(img).collect();
    let similar = ext_cards(
        &app,
        &v,
        "e.id IN (SELECT value FROM json_each(?2))",
        "(SELECT key FROM json_each(?2) j WHERE j.value = e.id)",
        &[&x["similar"].to_string()],
    )?;
    let local: Vec<Card> =
        card.local.map(|t| cards_by_ids(&app, v.profile.kids, &[t])).transpose()?.unwrap_or_default();
    let list = |k: &str| x[k].as_array().cloned().unwrap_or_default();
    let refreshing = {
        let jobs = app.detail_jobs.lock();
        jobs.contains(&id) || jobs.contains(&format!("rich:{id}"))
    };
    Ok(Json(json!({
        "card": card, "overview": overview, "runtime": runtime, "votes": votes,
        "refreshing": refreshing,
        "backdrop": crate::api::img(backdrop.filter(|b| !b.is_empty())),
        "posterLarge": img(&x["poster"]),
        "tagline": x["tagline"], "age": x["age"], "releaseDate": x["releaseDate"],
        "countries": list("countries"), "languages": list("languages"), "aka": list("aka"),
        "people": people, "gallery": gallery, "similar": similar,
        "localCard": local.first(),
        "lists": super::lists::of_ext(&app, &v, &id)?,
        "links": super::links::of(&app, v.profile.id, None, Some(&id))?,
        "seasons": list("seasons"),
        "friends": super::activity::of_title(&app, &v, None, Some(&id))?,
    })))
}

#[derive(Deserialize)]
pub struct SeasonQuery {
    season: Option<i32>,
}

/// Серии сезона сериала из ленты (с IMDb - один раз в неделю) и какие из них я отметил.
pub async fn episodes(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Query(q): Query<SeasonQuery>,
) -> ApiResult<Json<Value>> {
    let card = ext_card(&app, &v, &id)?;
    if card.kind != "series" {
        return Err(ApiError::not_found());
    }
    let extra: Option<String> =
        app.db.with(|c| c.query_row("SELECT extra FROM ext_titles WHERE id = ?", [&id], |r| r.get(0)))?;
    let x: Value = extra.and_then(|e| serde_json::from_str(&e).ok()).unwrap_or(Value::Null);
    let seasons: Vec<i32> =
        x["seasons"].as_array().into_iter().flatten().filter_map(|s| s.as_i64().map(|n| n as i32)).collect();
    let season = q.season.or_else(|| seasons.first().copied()).unwrap_or(1);
    let job = format!("episodes:{id}:{season}");
    let last = app.db.setting_or(&job, "0").parse::<i64>().unwrap_or(0);
    if !discover::season_fresh(&app, &id, season)
        && now_ms() - last > 60_000
        && app.detail_jobs.lock().insert(job.clone())
    {
        let _ = app.db.set_setting(&job, &now_ms().to_string());
        let a = app.clone();
        let i = id.clone();
        tokio::spawn(async move {
            let (primary, _) = tokio::join!(
                tokio::time::timeout(Duration::from_secs(5), discover::season_data(&a, &i, season)),
                tokio::time::timeout(Duration::from_secs(5), crate::media_meta::episodes(&a, &i, season))
            );
            let has_episodes =
                a.db.with(|c| {
                    c.query_row(
                        "SELECT EXISTS(SELECT 1 FROM ext_episodes WHERE ext_id=? AND season=?)",
                        params![i, season],
                        |r| r.get::<_, bool>(0),
                    )
                })
                .unwrap_or(false);
            if !matches!(primary, Ok(Ok(_))) && has_episodes {
                let _ = a.db.with(|c| {
                    c.execute(
                        "INSERT INTO ext_seasons VALUES(?,?,?) ON CONFLICT(ext_id,season) DO UPDATE SET at=excluded.at",
                        params![i, season, now_ms()],
                    )
                });
            }
            a.detail_jobs.lock().remove(&job);
        });
    }
    let mut items: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT e.episode, e.name, e.overview, COALESCE(NULLIF(e.still, ''),e.still_url), e.air_date, e.runtime, e.rating,
                    EXISTS (SELECT 1 FROM ext_seen s WHERE s.profile_id = ?3 AND s.ext_id = e.ext_id
                              AND s.season = e.season AND s.episode = e.episode)
             FROM ext_episodes e WHERE e.ext_id = ?1 AND e.season = ?2 ORDER BY e.episode",
        )?;
        let r = st.query_map(params![id, season, v.profile.id], |r| {
            Ok(json!({
                "season": season, "episode": r.get::<_, i32>(0)?, "name": r.get::<_, Option<String>>(1)?,
                "overview": r.get::<_, Option<String>>(2)?, "still": r.get::<_,Option<String>>(3)?.map(|s|if s.starts_with("https://"){crate::media_meta::art(&s)}else{format!("/img/{s}")}),
                "airDate": r.get::<_, Option<String>>(4)?, "runtime": r.get::<_, Option<i64>>(5)?,
                "rating": r.get::<_, Option<f64>>(6)?, "seen": r.get::<_, bool>(7)?,
            }))
        })?;
        r.collect()
    })?;
    let texts: Vec<String> =
        items.iter().flat_map(|e| [e["name"].as_str(), e["overview"].as_str()]).flatten().map(str::to_owned).collect();
    crate::episode_locale::queue(&app, texts.clone());
    let localizing = texts.iter().any(|text| app.detail_jobs.lock().contains(&crate::episode_locale::key(text)));
    for episode in &mut items {
        episode["name"] =
            json!(crate::episode_locale::localized(&app, episode["name"].as_str()).unwrap_or_else(|| "Серия".into()));
        episode["overview"] = json!(crate::episode_locale::localized(&app, episode["overview"].as_str()));
    }
    Ok(Json(
        json!({"seasons": seasons, "season": season, "items": items,"refreshing":localizing || app.detail_jobs.lock().contains(&format!("episodes:{id}:{season}"))}),
    ))
}

/// Оценки всех серий сериала по сезонам (сетка «как на SeriesGraph»). `id` - IMDb: и у сериала
/// из ленты, и у своего (его `imdbId`). Сезоны - с IMDb один раз в неделю, по три разом.
pub async fn ratings(State(app): State<Arc<App>>, _v: Viewer, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    if !discover::valid_id(&id) {
        return Err(ApiError::not_found());
    }
    // Своего сериала может не быть в ленте - заводим строку (серии ссылаются на неё).
    let local: Option<String> = app
        .db
        .with(|c| c.query_row("SELECT title FROM titles WHERE imdb_id = ? LIMIT 1", [&id], |r| r.get(0)).optional())?;
    app.db.with(|c| {
        c.execute(
            "INSERT OR IGNORE INTO ext_titles (id, kind, title, updated_at) VALUES (?, 'series', ?, ?)",
            params![id, local.unwrap_or_else(|| id.clone()), now_ms()],
        )
    })?;
    let extra: Option<String> =
        app.db.with(|c| c.query_row("SELECT extra FROM ext_titles WHERE id = ?", [&id], |r| r.get(0)))?;
    let x: Value = extra.and_then(|e| serde_json::from_str(&e).ok()).unwrap_or(Value::Null);
    let mut seasons: Vec<i32> =
        x["seasons"].as_array().into_iter().flatten().filter_map(|s| s.as_i64().map(|n| n as i32)).collect();
    if seasons.is_empty() {
        let q = r#"query S($id: ID!) { title(id: $id) { episodes { seasons { number } } } }"#;
        if let Ok(v) = Imdb::new(app.http.clone()).data(q, json!({ "id": id }), "title").await {
            seasons = v["episodes"]["seasons"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s["number"].as_i64().map(|n| n as i32))
                .filter(|n| *n > 0)
                .collect();
        }
    }
    seasons.truncate(40);
    let work = futures_util::stream::iter(seasons.clone())
        .map(|s| {
            let (a, i) = (app.clone(), id.clone());
            async move {
                let _ = discover::season_data(&a, &i, s).await;
            }
        })
        .buffer_unordered(3)
        .collect::<Vec<_>>();
    let _ = tokio::time::timeout(Duration::from_secs(30), work).await;
    let rows: Vec<(i32, i32, Option<f64>, Option<String>)> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT season, episode, rating, name FROM ext_episodes WHERE ext_id = ? AND season > 0 ORDER BY season, episode",
        )?;
        let r = st.query_map([&id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        r.collect()
    })?;
    let mut out: Vec<Value> = Vec::new();
    for s in &seasons {
        let eps: Vec<Value> = rows
            .iter()
            .filter(|r| r.0 == *s && r.1 > 0)
            .map(|r| json!({"episode": r.1, "rating": r.2, "name": crate::episode_locale::localized(&app, r.3.as_deref()).unwrap_or_else(|| "Серия".into())}))
            .collect();
        if !eps.is_empty() {
            out.push(json!({"season": s, "episodes": eps}));
        }
    }
    Ok(Json(json!({"seasons": out})))
}

#[derive(Deserialize)]
pub struct SeenBody {
    season: i32,
    episode: i32,
    /// true - «досюда посмотрел» (эта и все раньше, с прошлыми сезонами), false - снять отметку с этой.
    value: bool,
}

/// Отметить серию сериала из ленты: сериал сам становится «Смотрю» (если не «Просмотрел»).
pub async fn seen(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Json(b): Json<SeenBody>,
) -> ApiResult<Json<Value>> {
    let card = ext_card(&app, &v, &id)?;
    if card.kind != "series" {
        return Err(ApiError::not_found());
    }
    let (me, now) = (v.profile.id, now_ms());
    if b.value {
        // Серии прошлых сезонов могли ещё не загружаться (сезон не открывали) - подтянуть с IMDb.
        for s in 1..b.season.min(60) {
            if let Err(e) = crate::discover::season_data(&app, &id, s).await {
                tracing::debug!(error = %e, season = s, "серии сезона не загрузились");
            }
        }
    }
    app.db.tx(|c| {
        if b.value {
            c.execute("DELETE FROM ext_dropped WHERE profile_id = ? AND ext_id = ?", params![me, id])?;
            c.execute(
                "INSERT OR IGNORE INTO ext_seen (profile_id, ext_id, season, episode, at)
                 SELECT ?1, ?2, season, episode, ?5 FROM ext_episodes
                 WHERE ext_id = ?2 AND ((season > 0 AND season < ?3) OR (season = ?3 AND episode <= ?4))",
                params![me, id, b.season, b.episode, now],
            )?;
            c.execute(
                "INSERT OR IGNORE INTO ext_seen (profile_id, ext_id, season, episode, at) VALUES (?, ?, ?, ?, ?)",
                params![me, id, b.season, b.episode, now],
            )?;
            if card.mark.as_deref() != Some("watched") {
                c.execute(
                    "INSERT INTO ext_marks (profile_id, ext_id, status, at) VALUES (?, ?, 'watching', ?)
                     ON CONFLICT(profile_id, ext_id) DO UPDATE SET status = 'watching', at = excluded.at",
                    params![me, id, now],
                )?;
            }
        } else {
            c.execute(
                "DELETE FROM ext_seen WHERE profile_id = ? AND ext_id = ? AND season = ? AND episode = ?",
                params![me, id, b.season, b.episode],
            )?;
        }
        Ok(())
    })?;
    Ok(Json(json!({"ok": true, "mark": ext_card(&app, &v, &id)?.mark})))
}

#[derive(Deserialize)]
pub struct MarkBody {
    status: Option<String>,
}

pub async fn mark(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Json(b): Json<MarkBody>,
) -> ApiResult<Json<Value>> {
    ext_card(&app, &v, &id)?;
    if b.status.as_deref() == Some("dropped") {
        app.db.tx(|c| {
            c.execute(
                "INSERT INTO ext_dropped VALUES (?1, ?2, ?3) ON CONFLICT(profile_id, ext_id) DO UPDATE SET at = ?3",
                params![v.profile.id, id, now_ms()],
            )?;
            c.execute("DELETE FROM ext_marks WHERE profile_id = ? AND ext_id = ?", params![v.profile.id, id])?;
            Ok(())
        })?;
        return Ok(Json(json!({"ok": true})));
    }
    if matches!(b.status.as_deref(), None | Some("want" | "watching" | "watched")) {
        app.db.with(|c| {
            c.execute("DELETE FROM ext_dropped WHERE profile_id = ? AND ext_id = ?", params![v.profile.id, id])
        })?;
    }
    match b.status.as_deref() {
        Some(s @ ("want" | "watching" | "watched")) => app.db.with(|c| {
            c.execute(
                "INSERT INTO ext_marks (profile_id, ext_id, status, at) VALUES (?, ?, ?, ?)
                 ON CONFLICT(profile_id, ext_id) DO UPDATE SET status = excluded.status, at = excluded.at",
                params![v.profile.id, id, s, now_ms()],
            )
        })?,
        None => app.db.with(|c| {
            c.execute("DELETE FROM ext_marks WHERE profile_id = ? AND ext_id = ?", params![v.profile.id, id])
        })?,
        Some(_) => return Err(ApiError::bad("bad_status", "Отметка - «want», «watching» или «watched»")),
    };
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct Flag {
    value: bool,
}

/// «Хочу посмотреть на сервисе»: просьба хосту добавить фильм.
pub async fn wish(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<String>,
    Json(b): Json<Flag>,
) -> ApiResult<Json<Value>> {
    ext_card(&app, &v, &id)?;
    app.db.with(|c| {
        if b.value {
            c.execute(
                "INSERT INTO wishes (profile_id, ext_id, at) VALUES (?, ?, ?)
                 ON CONFLICT(profile_id, ext_id) DO UPDATE SET at = excluded.at, dismissed_at = NULL",
                params![v.profile.id, id, now_ms()],
            )
        } else {
            c.execute("DELETE FROM wishes WHERE profile_id = ? AND ext_id = ?", params![v.profile.id, id])
        }
    })?;
    Ok(Json(json!({"ok": true})))
}

/// Мои «Буду смотреть» и «Просмотрел»: и с сервиса, и из ленты - одним списком, новое сверху.
pub async fn my_marks(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let me = v.profile.id;
    let mut out = json!({});
    let taste = crate::taste::Taste::marks(&app, me)?;
    for status in ["want", "watching", "watched"] {
        let local_sql = match status {
            "want" => {
                "SELECT l.title_id, l.added_at FROM my_list l WHERE l.profile_id = ?1
                   AND NOT EXISTS (SELECT 1 FROM watched w WHERE w.profile_id = ?1 AND w.title_id = l.title_id)"
            }
            "watching" => "SELECT title_id, at FROM watching WHERE profile_id = ?1",
            _ => "SELECT title_id, at FROM watched WHERE profile_id = ?1",
        };
        let mut local: Vec<(i64, i64)> = app.db.with(|c| {
            let mut st = c.prepare(local_sql)?;
            let r = st.query_map([me], |r| Ok((r.get(0)?, r.get(1)?)))?;
            r.collect()
        })?;
        if status == "watching" {
            // И начатое с сервиса (не брошенное и не досмотренное).
            for id in &taste.recent {
                if !local.iter().any(|l| l.0 == *id) {
                    local.push((*id, taste.last.get(id).copied().unwrap_or(0)));
                }
            }
        }
        local.retain(|l| taste.status(l.0) == Some(status) || status == "want");
        let ids: Vec<i64> = local.iter().map(|l| l.0).collect();
        let cards = cards_by_ids(&app, v.profile.kids, &ids)?;
        let mut items: Vec<(i64, Value)> = cards
            .into_iter()
            .filter_map(|c| {
                let at = local.iter().find(|l| l.0 == c.id)?.1;
                Some((at, json!({"at": at, "card": c})))
            })
            .collect();
        let ext = ext_cards(
            &app,
            &v,
            "e.id IN (SELECT ext_id FROM ext_marks WHERE profile_id = ?1 AND status = ?2)",
            "e.id",
            &[&status],
        )?;
        for e in ext {
            let at: i64 = app.db.with(|c| {
                c.query_row("SELECT at FROM ext_marks WHERE profile_id = ? AND ext_id = ?", params![me, e.id], |r| {
                    r.get(0)
                })
            })?;
            items.push((at, json!({"at": at, "ext": e})));
        }
        items.sort_by_key(|i| std::cmp::Reverse(i.0));
        out[status] = Value::Array(items.into_iter().map(|i| i.1).collect());
    }
    out["favorites"] = super::profile::decor(&app, &v, me)?["favorites"].clone();
    Ok(Json(out))
}

// ---------- просьбы у хоста ----------

/// Что просят добавить: фильм, сколько человек и кто. Уже появившееся на сервисе - не показываем.
pub async fn wishes(State(app): State<Arc<App>>, m: Manager) -> ApiResult<Json<Value>> {
    let v = m.0;
    let rows: Vec<(String, i64, String, i64)> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT w.ext_id, COUNT(*), GROUP_CONCAT(p.name, ', '), MAX(w.at) FROM wishes w
             JOIN profiles p ON p.id = w.profile_id
             WHERE w.dismissed_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM titles t WHERE t.imdb_id = w.ext_id AND t.hidden = 0
                 AND EXISTS (SELECT 1 FROM files f WHERE f.title_id = t.id AND f.missing = 0))
             GROUP BY w.ext_id ORDER BY COUNT(*) DESC, MAX(w.at) DESC",
        )?;
        let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        r.collect()
    })?;
    let mut out = Vec::new();
    for (id, count, people, at) in rows {
        if let Ok(card) = ext_card(&app, &v, &id) {
            out.push(json!({"card": card, "count": count, "people": people, "at": at}));
        }
    }
    Ok(Json(json!(out)))
}

pub fn open_wishes(app: &App) -> rusqlite::Result<i64> {
    app.db.with(|c| {
        c.query_row(
            "SELECT COUNT(DISTINCT w.ext_id) FROM wishes w WHERE w.dismissed_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM titles t WHERE t.imdb_id = w.ext_id AND t.hidden = 0
                 AND EXISTS (SELECT 1 FROM files f WHERE f.title_id = t.id AND f.missing = 0))",
            [],
            |r| r.get(0),
        )
    })
}

/// Хост отклонил или уже добавил - просьба закрыта у всех.
pub async fn dismiss_wish(State(app): State<Arc<App>>, _m: Manager, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    app.db.with(|c| {
        c.execute("UPDATE wishes SET dismissed_at = ? WHERE ext_id = ? AND dismissed_at IS NULL", params![now_ms(), id])
    })?;
    Ok(Json(json!({"ok": true})))
}

/// Обновить подборки сейчас (хост).
pub async fn refresh(State(app): State<Arc<App>>, _m: Manager) -> ApiResult<Json<Value>> {
    let n = discover::refresh(&app).await.map_err(|e| ApiError::bad("imdb", format!("IMDb не ответил: {e}")))?;
    Ok(Json(json!({"ok": true, "charts": n})))
}

// ---------- стена постеров на странице входа ----------

/// Постеры популярного с IMDb для фона страницы входа - без входа. Только из подборок IMDb:
/// что лежит у хоста, до входа не видно.
pub async fn wall(State(app): State<Arc<App>>) -> ApiResult<Json<Value>> {
    let names: Vec<String> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT e.poster FROM ext_titles e JOIN ext_charts ch ON ch.ext_id = e.id
             WHERE e.poster IS NOT NULL AND e.poster != ''
             GROUP BY e.id ORDER BY MIN(ch.pos), e.id LIMIT 36",
        )?;
        let r = st.query_map([], |r| r.get(0))?;
        r.collect()
    })?;
    Ok(Json(json!(names.into_iter().map(|n| format!("/api/site/wall/{n}")).collect::<Vec<_>>())))
}

/// Картинка стены: отдаём, только если это постер из подборок IMDb.
pub async fn wall_image(State(app): State<Arc<App>>, Path(name): Path<String>) -> ApiResult<axum::response::Response> {
    use axum::http::header;
    use axum::response::IntoResponse;
    let known: bool = app.db.with(|c| {
        c.query_row(
            "SELECT EXISTS (SELECT 1 FROM ext_titles e JOIN ext_charts ch ON ch.ext_id = e.id WHERE e.poster = ?)",
            [&name],
            |r| r.get(0),
        )
    })?;
    let path = app.images.path(&name).filter(|_| known).ok_or_else(ApiError::not_found)?;
    let bytes = tokio::fs::read(&path).await?;
    let mime = match name.rsplit('.').next() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "public, max-age=86400")], bytes).into_response())
}

#[derive(Deserialize)]
pub struct DirectorQuery {
    name: String,
    /// Id на IMDb (если известен из описания фильма - без поиска).
    id: Option<String>,
    /// director или actor.
    role: Option<String>,
}

/// Страница человека (режиссёр или актёр): фото и фильмография с IMDb (раз в неделю), что из этого
/// есть на Nova.
pub async fn director(
    State(app): State<Arc<App>>,
    v: Viewer,
    Query(q): Query<DirectorQuery>,
) -> ApiResult<Json<Value>> {
    let name = q.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::not_found());
    }
    let role = crate::directors::role(q.role.as_deref().unwrap_or("director"));
    let given = q.id.filter(|i| i.starts_with("nm") && i.len() <= 12 && i[2..].bytes().all(|b| b.is_ascii_digit()));
    let id = match given {
        Some(i) => Some(i),
        None => tokio::time::timeout(Duration::from_secs(15), crate::directors::resolve(&app, name))
            .await
            .ok()
            .and_then(|r| r.ok())
            .flatten(),
    };
    let mut info = Value::Null;
    let mut imdb_ok = false;
    if let Some(id) = &id {
        let (imdb, extra) = tokio::join!(
            tokio::time::timeout(Duration::from_secs(25), crate::directors::fetch(&app, id, role)),
            tokio::time::timeout(Duration::from_secs(24), crate::directors::information(&app, id, name, role))
        );
        imdb_ok = matches!(imdb, Ok(Ok(())));
        if let Ok(Ok(data)) = extra {
            info = data;
        }
    }
    type Row = (String, Option<String>, String);
    let row: Option<Row> = match &id {
        Some(id) => app.db.with(|c| {
            c.query_row("SELECT name, photo, works FROM ext_people WHERE id = ? AND role = ?", params![id, role], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()
        })?,
        None => None,
    };
    let mut ids: Vec<String> = row.as_ref().and_then(|r| serde_json::from_str(&r.2).ok()).unwrap_or_default();
    for value in info["works"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        if !ids.iter().any(|i| i == value) {
            ids.push(value.into());
        }
    }
    let works_json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
    let works = ext_cards(
        &app,
        &v,
        "e.id IN (SELECT value FROM json_each(?2))",
        "(SELECT key FROM json_each(?2) j WHERE j.value = e.id)",
        &[&works_json],
    )?;
    // Своё: снятое им (по имени в описании) и то, что есть в его фильмографии.
    let mut local = super::cards(
        &app,
        v.profile.kids,
        "t.imdb_id IN (SELECT value FROM json_each(?1)) OR EXISTS (SELECT 1 FROM json_each(t.people) p
           WHERE json_extract(p.value, '$.name') = ?2 AND json_extract(p.value, '$.job') IN (?3, ?4))",
        "t.year DESC",
        60,
        &[
            &works_json,
            &name,
            &(if role == "actor" { "actor" } else { "director" }),
            &(if role == "actor" { "actor" } else { "creator" }),
        ],
    )?;
    crate::taste::Taste::load(&app, v.profile.id)?.apply(&mut local);
    Ok(Json(json!({
        "name": name, "imdbName": row.as_ref().map(|r| r.0.clone()), "id": id, "role": role,
        "photo": super::img(row.and_then(|r| r.1).or_else(|| info["photo"].as_str().map(String::from))), "works": works, "local": local,
        "info": info, "availability": if imdb_ok { "full" } else if !ids.is_empty() { "partial" } else { "unavailable" },
    })))
}

#[derive(Deserialize)]
pub struct FranchiseQuery {
    imdb: String,
}

/// Хронология франшиз фильма: серии (узкие первыми) с карточками по порядку.
pub async fn franchise(
    State(app): State<Arc<App>>,
    v: Viewer,
    Query(q): Query<FranchiseQuery>,
) -> ApiResult<Json<Value>> {
    let groups = crate::franchise::of(&app, q.imdb.trim()).await;
    let mut out = Vec::new();
    for g in groups {
        let items: Vec<ExtCard> = g.ids.iter().filter_map(|id| ext_card(&app, &v, id).ok()).collect();
        if items.len() > 1 {
            out.push(json!({"name": g.name, "items": items}));
        }
    }
    Ok(Json(json!(out)))
}

#[cfg(test)]
mod rank_tests {
    use super::search_rank;

    #[test]
    fn exact_original_title_first() {
        let q = crate::util::norm("Fight Club");
        let real = search_rank(&q, "Бойцовский клуб", Some("Fight Club"), Some(8.8));
        let other = search_rank(&q, "Superhero Fight Club", None, None);
        assert!(real < other, "настоящий фильм - выше");
        let better = search_rank(&q, "Fight Club", None, Some(9.0));
        let worse = search_rank(&q, "Fight Club", None, Some(5.0));
        assert!(better < worse, "при равенстве - выше оценка");
    }
}
