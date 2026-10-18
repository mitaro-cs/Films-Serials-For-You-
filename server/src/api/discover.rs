//! Лента IMDb для интерфейса: подборки, поиск по всему IMDb, карточка фильма не с сервиса,
//! «Буду смотреть»/«Просмотрел», «Хочу на сервисе» (просьба хосту) и список просьб у хоста.

use super::{cards_by_ids, Card};
use crate::auth::{Manager, Viewer};
use crate::discover::{self, card, card_sql, ExtCard, CHARTS};
use crate::error::{ApiError, ApiResult};
use crate::meta::imdb::Imdb;
use crate::util::now_ms;
use crate::App;
use axum::extract::{Path, Query, State};
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

const ROW: i64 = 40;

fn ext_cards(app: &App, v: &Viewer, filter: &str, order: &str, p: &[&dyn rusqlite::ToSql]) -> ApiResult<Vec<ExtCard>> {
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

/// Подборки ленты. Пусто (ещё не загружали) — просим обновить.
pub async fn feed(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let mut rows = Vec::new();
    for (key, _, title) in CHARTS {
        let items = ext_cards(
            &app,
            &v,
            &format!("e.id IN (SELECT ext_id FROM ext_charts WHERE chart = ?2 AND pos < {ROW})"),
            "(SELECT pos FROM ext_charts ch WHERE ch.chart = ?2 AND ch.ext_id = e.id)",
            &[key],
        )?;
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

/// Вся подборка целиком (например, топ-250).
pub async fn chart(State(app): State<Arc<App>>, v: Viewer, Path(key): Path<String>) -> ApiResult<Json<Value>> {
    let title = CHARTS.iter().find(|c| c.0 == key).map(|c| c.2).ok_or_else(ApiError::not_found)?;
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
}

/// Поиск по всему IMDb: найденное сохраняется в ленту, постеры — скачиваются сразу (до 6 с).
pub async fn search(State(app): State<Arc<App>>, v: Viewer, Query(q): Query<SearchQuery>) -> ApiResult<Json<Value>> {
    let query = q.q.trim();
    if query.chars().count() < 2 {
        return Ok(Json(json!([])));
    }
    let found = Imdb::new(app.http.clone())
        .search(query, None, false)
        .await
        .map_err(|e| ApiError::bad("imdb", format!("IMDb не ответил: {e}")))?;
    let items: Vec<discover::Ext> = found
        .into_iter()
        .filter(|c| discover::valid_id(&c.id))
        .take(12)
        .map(|c| discover::Ext {
            id: c.id,
            series: c.series,
            title: c.title,
            original: c.original,
            year: c.year,
            rating: None,
            votes: None,
            genres: vec![],
            poster_url: c.poster,
        })
        .collect();
    discover::upsert(&app, &items)?;
    let _ = tokio::time::timeout(Duration::from_secs(6), discover::posters_batch(&app, 12)).await;
    let ids: Vec<String> = items.iter().map(|e| e.id.clone()).collect();
    let mut out = Vec::new();
    for id in &ids {
        if let Ok(c) = ext_card(&app, &v, id) {
            out.push(c);
        }
    }
    Ok(Json(json!(out)))
}

/// Карточка фильма из ленты: описание (из IMDb, один раз), мои списки, ссылки на видео.
pub async fn title(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    if !discover::valid_id(&id) {
        return Err(ApiError::not_found());
    }
    let known: Option<bool> =
        app.db.with(|c| c.query_row("SELECT detailed FROM ext_titles WHERE id = ?", [&id], |r| r.get(0)).optional())?;
    if known != Some(true) {
        if let Ok(d) = Imdb::new(app.http.clone()).details(&id, &[]).await {
            let e = discover::Ext {
                id: id.clone(),
                series: d.series,
                title: d.title.clone(),
                original: d.original.clone(),
                year: d.year,
                rating: d.rating,
                votes: d.votes,
                genres: d.genres.iter().map(|g| crate::meta::genre_ru(g)).collect(),
                poster_url: d.poster.clone(),
            };
            discover::upsert(&app, &[e])?;
            app.db.with(|c| {
                c.execute(
                    "UPDATE ext_titles SET overview = ?, runtime = ?, detailed = 1 WHERE id = ?",
                    params![d.overview, d.runtime, id],
                )
            })?;
            let _ = tokio::time::timeout(Duration::from_secs(5), discover::posters_batch(&app, 1)).await;
        } else if known.is_none() {
            return Err(ApiError::bad("imdb", "IMDb сейчас не отвечает — попробуйте позже"));
        }
    }
    let card = ext_card(&app, &v, &id)?;
    let (overview, runtime, votes): (Option<String>, Option<i64>, Option<i64>) = app.db.with(|c| {
        c.query_row("SELECT overview, runtime, votes FROM ext_titles WHERE id = ?", [&id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
    })?;
    let local: Vec<Card> =
        card.local.map(|t| cards_by_ids(&app, v.profile.kids, &[t])).transpose()?.unwrap_or_default();
    Ok(Json(json!({
        "card": card, "overview": overview, "runtime": runtime, "votes": votes,
        "localCard": local.first(),
        "lists": super::lists::of_ext(&app, &v, &id)?,
        "links": super::links::of(&app, None, Some(&id))?,
    })))
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
    match b.status.as_deref() {
        Some(s @ ("want" | "watched")) => app.db.with(|c| {
            c.execute(
                "INSERT INTO ext_marks (profile_id, ext_id, status, at) VALUES (?, ?, ?, ?)
                 ON CONFLICT(profile_id, ext_id) DO UPDATE SET status = excluded.status, at = excluded.at",
                params![v.profile.id, id, s, now_ms()],
            )
        })?,
        None => app.db.with(|c| {
            c.execute("DELETE FROM ext_marks WHERE profile_id = ? AND ext_id = ?", params![v.profile.id, id])
        })?,
        Some(_) => return Err(ApiError::bad("bad_status", "Отметка — «want» или «watched»")),
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

/// Мои «Буду смотреть» и «Просмотрел»: и с сервиса, и из ленты — одним списком, новое сверху.
pub async fn my_marks(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    let me = v.profile.id;
    let mut out = json!({});
    for status in ["want", "watched"] {
        let local_sql = if status == "want" {
            "SELECT l.title_id, l.added_at FROM my_list l WHERE l.profile_id = ?1
               AND NOT EXISTS (SELECT 1 FROM watched w WHERE w.profile_id = ?1 AND w.title_id = l.title_id)"
        } else {
            "SELECT title_id, at FROM watched WHERE profile_id = ?1"
        };
        let local: Vec<(i64, i64)> = app.db.with(|c| {
            let mut st = c.prepare(local_sql)?;
            let r = st.query_map([me], |r| Ok((r.get(0)?, r.get(1)?)))?;
            r.collect()
        })?;
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
    Ok(Json(out))
}

// ---------- просьбы у хоста ----------

/// Что просят добавить: фильм, сколько человек и кто. Уже появившееся на сервисе — не показываем.
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

/// Хост отклонил или уже добавил — просьба закрыта у всех.
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
