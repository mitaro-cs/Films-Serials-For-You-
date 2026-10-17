//! Люди: отзывы и комментарии к фильмам (с оценкой по желанию), участники, друзья и переписка
//! между друзьями (можно приложить фильм — «обсудить»). Действуют профили: у каждого человека в
//! семейном аккаунте свои друзья и отзывы. Детские профили в общении не участвуют.

use super::{card, img, visible, CARD_COLS};
use crate::auth::Viewer;
use crate::error::{ApiError, ApiResult};
use crate::util::now_ms;
use crate::App;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

const MAX_REVIEW: usize = 4000;
const MAX_MESSAGE: usize = 2000;
const PAGE: i64 = 50;

/// Автор отзыва, участник, собеседник.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: i64,
    pub name: String,
    pub avatar: i64,
    pub photo: Option<String>,
}

const PERSON_COLS: &str = "p.id, p.name, p.avatar, p.photo";

fn person(r: &Row, at: usize) -> rusqlite::Result<Person> {
    Ok(Person { id: r.get(at)?, name: r.get(at + 1)?, avatar: r.get(at + 2)?, photo: img(r.get(at + 3)?) })
}

/// Профили, видимые в общении: не детские, аккаунт не отключён.
const SOCIAL: &str = "p.kids = 0 AND EXISTS (SELECT 1 FROM users u WHERE u.id = p.user_id AND u.disabled = 0)";

fn adult(v: &Viewer) -> ApiResult<()> {
    if v.profile.kids {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "kids", "В детском профиле общение выключено"));
    }
    Ok(())
}

fn social_profile(app: &App, id: i64) -> ApiResult<Person> {
    app.db
        .with(|c| {
            c.query_row(&format!("SELECT {PERSON_COLS} FROM profiles p WHERE p.id = ? AND {SOCIAL}"), [id], |r| {
                person(r, 0)
            })
            .optional()
        })?
        .ok_or_else(ApiError::not_found)
}

/// Текст: без пустых строк по краям, не длиннее предела.
fn text(s: &str, max: usize, what: &str) -> ApiResult<String> {
    let t = s.trim();
    if t.chars().count() > max {
        return Err(ApiError::bad("too_long", format!("{what} — не длиннее {max} символов")));
    }
    Ok(t.to_string())
}

// ---------- отзывы и комментарии ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub id: i64,
    pub author: Person,
    /// 1..10 — половинки звезды; None — просто комментарий.
    pub stars: Option<i32>,
    pub body: String,
    pub created_at: i64,
    pub edited_at: Option<i64>,
    pub likes: i64,
    pub liked: bool,
    pub mine: bool,
}

fn review_sql(filter: &str) -> String {
    format!(
        "SELECT r.id, r.stars, r.body, r.created_at, r.edited_at,
                (SELECT COUNT(*) FROM review_likes l WHERE l.review_id = r.id),
                EXISTS (SELECT 1 FROM review_likes l WHERE l.review_id = r.id AND l.profile_id = ?1),
                {PERSON_COLS}
         FROM reviews r JOIN profiles p ON p.id = r.profile_id
         WHERE {filter} ORDER BY r.created_at DESC, r.id DESC LIMIT {PAGE}"
    )
}

fn review(r: &Row, me: i64) -> rusqlite::Result<Review> {
    let author = person(r, 7)?;
    Ok(Review {
        id: r.get(0)?,
        stars: r.get(1)?,
        body: r.get(2)?,
        created_at: r.get(3)?,
        edited_at: r.get(4)?,
        likes: r.get(5)?,
        liked: r.get(6)?,
        mine: author.id == me,
        author,
    })
}

fn title_visible(app: &App, v: &Viewer, id: i64) -> ApiResult<()> {
    let ok: bool = app.db.with(|c| {
        c.query_row(
            &format!("SELECT EXISTS (SELECT 1 FROM titles t WHERE t.id = ? AND {})", visible(v.profile.kids)),
            [id],
            |r| r.get(0),
        )
    })?;
    if ok {
        Ok(())
    } else {
        Err(ApiError::not_found())
    }
}

/// Средняя оценка: последняя оценка каждого человека; звёзды 0,5..5.
fn rating_of(app: &App, title: i64) -> rusqlite::Result<(Option<f64>, i64)> {
    app.db.with(|c| {
        c.query_row(
            "SELECT AVG(stars) / 2.0, COUNT(*) FROM (
               SELECT stars FROM reviews r WHERE title_id = ?1 AND stars IS NOT NULL
                 AND r.id = (SELECT MAX(id) FROM reviews x WHERE x.title_id = ?1 AND x.profile_id = r.profile_id AND x.stars IS NOT NULL))",
            [title],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
    })
}

pub async fn reviews(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    title_visible(&app, &v, id)?;
    let items: Vec<Review> = app.db.with(|c| {
        let mut st = c.prepare(&review_sql(&format!("r.title_id = ?2 AND {SOCIAL}")))?;
        let r = st.query_map(params![v.profile.id, id], |r| review(r, v.profile.id))?;
        r.collect()
    })?;
    let (avg, votes) = rating_of(&app, id)?;
    let total: i64 =
        app.db.with(|c| c.query_row("SELECT COUNT(*) FROM reviews WHERE title_id = ?", [id], |r| r.get(0)))?;
    Ok(Json(json!({ "average": avg, "votes": votes, "total": total, "items": items, "canWrite": !v.profile.kids })))
}

#[derive(Deserialize)]
pub struct ReviewBody {
    #[serde(default)]
    body: String,
    stars: Option<i32>,
}

fn check_review(b: &ReviewBody) -> ApiResult<String> {
    let body = text(&b.body, MAX_REVIEW, "Отзыв")?;
    if let Some(s) = b.stars {
        if !(1..=10).contains(&s) {
            return Err(ApiError::bad("bad_stars", "Оценка — от половины до пяти звёзд"));
        }
    }
    if body.is_empty() && b.stars.is_none() {
        return Err(ApiError::bad("empty", "Напишите пару слов или поставьте оценку"));
    }
    Ok(body)
}

pub async fn add_review(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<ReviewBody>,
) -> ApiResult<Json<Review>> {
    adult(&v)?;
    title_visible(&app, &v, id)?;
    let body = check_review(&b)?;
    let me = v.profile.id;
    let review = app.db.with(|c| {
        c.execute(
            "INSERT INTO reviews (title_id, profile_id, stars, body, created_at) VALUES (?, ?, ?, ?, ?)",
            params![id, me, b.stars, body, now_ms()],
        )?;
        let rid = c.last_insert_rowid();
        c.query_row(&review_sql("r.id = ?2"), params![me, rid], |r| review(r, me))
    })?;
    Ok(Json(review))
}

/// Свой отзыв — править и удалять; чужой — удалять может хост или администратор.
fn own_review(app: &App, v: &Viewer, id: i64, moderate: bool) -> ApiResult<()> {
    let author: Option<i64> =
        app.db.with(|c| c.query_row("SELECT profile_id FROM reviews WHERE id = ?", [id], |r| r.get(0)).optional())?;
    match author {
        None => Err(ApiError::not_found()),
        Some(a) if a == v.profile.id => Ok(()),
        Some(_) if moderate && v.role != "viewer" => Ok(()),
        Some(_) => Err(ApiError::new(StatusCode::FORBIDDEN, "not_yours", "Это чужой отзыв")),
    }
}

pub async fn edit_review(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<ReviewBody>,
) -> ApiResult<Json<Review>> {
    own_review(&app, &v, id, false)?;
    let body = check_review(&b)?;
    let me = v.profile.id;
    let review = app.db.with(|c| {
        c.execute(
            "UPDATE reviews SET body = ?, stars = ?, edited_at = ? WHERE id = ?",
            params![body, b.stars, now_ms(), id],
        )?;
        c.query_row(&review_sql("r.id = ?2"), params![me, id], |r| review(r, me))
    })?;
    Ok(Json(review))
}

pub async fn delete_review(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    own_review(&app, &v, id, true)?;
    app.db.with(|c| c.execute("DELETE FROM reviews WHERE id = ?", [id]))?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct LikeBody {
    value: bool,
}

pub async fn like_review(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<LikeBody>,
) -> ApiResult<Json<Value>> {
    adult(&v)?;
    let n = app.db.with(|c| {
        if b.value {
            c.execute(
                "INSERT OR IGNORE INTO review_likes (review_id, profile_id) SELECT id, ? FROM reviews WHERE id = ?",
                params![v.profile.id, id],
            )?;
        } else {
            c.execute("DELETE FROM review_likes WHERE review_id = ? AND profile_id = ?", params![id, v.profile.id])?;
        }
        c.query_row("SELECT COUNT(*) FROM review_likes WHERE review_id = ?", [id], |r| r.get::<_, i64>(0))
    })?;
    Ok(Json(json!({"likes": n, "liked": b.value})))
}

// ---------- участники и друзья ----------

/// Отношение ко мне: none, friend, incoming (просит дружбы), outgoing (я просил).
fn relation(app: &App, me: i64, other: i64) -> rusqlite::Result<&'static str> {
    app.db.with(|c| {
        let row: Option<(i64, String)> = c
            .query_row(
                "SELECT a, status FROM friends WHERE (a = ?1 AND b = ?2) OR (a = ?2 AND b = ?1)",
                params![me, other],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            None => "none",
            Some((_, s)) if s == "accepted" => "friend",
            Some((a, _)) if a == me => "outgoing",
            Some(_) => "incoming",
        })
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    #[serde(flatten)]
    pub person: Person,
    pub relation: &'static str,
    pub watched: i64,
    pub reviews: i64,
}

fn members_of(app: &App, me: i64, filter: &str, p: &[&dyn rusqlite::ToSql]) -> ApiResult<Vec<Member>> {
    let rows: Vec<(Person, i64, i64)> = app.db.with(|c| {
        let mut st = c.prepare(&format!(
            "SELECT {PERSON_COLS},
                    (SELECT COUNT(*) FROM watched w WHERE w.profile_id = p.id),
                    (SELECT COUNT(*) FROM reviews r WHERE r.profile_id = p.id)
             FROM profiles p WHERE {SOCIAL} AND p.id != {me} AND ({filter}) ORDER BY p.name COLLATE NOCASE LIMIT 500"
        ))?;
        let r = st.query_map(p, |r| Ok((person(r, 0)?, r.get(4)?, r.get(5)?)))?;
        r.collect()
    })?;
    rows.into_iter()
        .map(|(person, watched, reviews)| {
            Ok(Member { relation: relation(app, me, person.id)?, person, watched, reviews })
        })
        .collect()
}

#[derive(Deserialize)]
pub struct MembersQuery {
    q: Option<String>,
}

pub async fn members(
    State(app): State<Arc<App>>,
    v: Viewer,
    Query(q): Query<MembersQuery>,
) -> ApiResult<Json<Vec<Member>>> {
    adult(&v)?;
    let mut list = members_of(&app, v.profile.id, "1", &[])?;
    if let Some(q) = q.q.as_deref().map(crate::util::norm).filter(|q| !q.is_empty()) {
        list.retain(|m| crate::util::norm(&m.person.name).contains(&q));
    }
    // Сначала просьбы дружбы, потом друзья, потом остальные.
    list.sort_by_key(|m| match m.relation {
        "incoming" => 0,
        "friend" => 1,
        "outgoing" => 2,
        _ => 3,
    });
    Ok(Json(list))
}

pub async fn member(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    adult(&v)?;
    let me = v.profile.id;
    let m = if id == me {
        None
    } else {
        Some(members_of(&app, me, "p.id = ?1", &[&id])?.pop().ok_or_else(ApiError::not_found)?)
    };
    let person = social_profile(&app, id)?;
    let reviews: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(&format!(
            "SELECT r.id, r.stars, r.body, r.created_at, {CARD_COLS}
             FROM reviews r JOIN titles t ON t.id = r.title_id
             WHERE r.profile_id = ?1 AND {} ORDER BY r.created_at DESC LIMIT 20",
            visible(false)
        ))?;
        let r = st.query_map([id], |r| {
            let c = card_at(r, 4)?;
            Ok(json!({"id": r.get::<_, i64>(0)?, "stars": r.get::<_, Option<i32>>(1)?, "body": r.get::<_, String>(2)?,
                      "createdAt": r.get::<_, i64>(3)?, "title": c}))
        })?;
        r.collect()
    })?;
    let watched: Vec<super::Card> = app.db.with(|c| {
        let mut st = c.prepare(&format!(
            "SELECT {CARD_COLS} FROM watched w JOIN titles t ON t.id = w.title_id
             WHERE w.profile_id = ? AND {} ORDER BY w.at DESC LIMIT 24",
            visible(false)
        ))?;
        let r = st.query_map([id], card)?;
        r.collect()
    })?;
    Ok(Json(json!({
        "person": person, "me": id == me, "relation": m.as_ref().map(|m| m.relation),
        "stats": {"watched": m.as_ref().map(|m| m.watched).unwrap_or(watched.len() as i64),
                  "reviews": m.as_ref().map(|m| m.reviews).unwrap_or(reviews.len() as i64)},
        "reviews": reviews, "watched": watched,
    })))
}

/// Карточка из колонок `CARD_COLS`, начиная с `at`.
fn card_at(r: &Row, at: usize) -> rusqlite::Result<Value> {
    Ok(json!({"id": r.get::<_, i64>(at)?, "kind": r.get::<_, String>(at + 1)?, "title": r.get::<_, String>(at + 2)?,
              "year": r.get::<_, Option<i32>>(at + 4)?, "poster": img(r.get(at + 5)?)}))
}

/// Подружиться: нет заявки — отправить; есть встречная — принять.
pub async fn befriend(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    adult(&v)?;
    let me = v.profile.id;
    if id == me {
        return Err(ApiError::bad("self", "Это вы"));
    }
    social_profile(&app, id)?;
    let rel = relation(&app, me, id)?;
    app.db.with(|c| match rel {
        "incoming" => {
            c.execute("UPDATE friends SET status = 'accepted' WHERE a = ? AND b = ?", params![id, me])?;
            Ok(())
        }
        "none" => {
            c.execute(
                "INSERT INTO friends (a, b, status, created_at) VALUES (?, ?, 'pending', ?)",
                params![me, id, now_ms()],
            )?;
            Ok(())
        }
        _ => Ok(()),
    })?;
    Ok(Json(json!({"relation": relation(&app, me, id)?})))
}

/// Отклонить, отменить заявку или убрать из друзей.
pub async fn unfriend(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    adult(&v)?;
    let me = v.profile.id;
    app.db
        .with(|c| c.execute("DELETE FROM friends WHERE (a = ?1 AND b = ?2) OR (a = ?2 AND b = ?1)", params![me, id]))?;
    Ok(Json(json!({"relation": "none"})))
}

// ---------- переписка ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub mine: bool,
    pub body: String,
    pub created_at: i64,
    pub read: bool,
    pub title: Option<Value>,
}

fn friends_only(app: &App, me: i64, other: i64) -> ApiResult<()> {
    if relation(app, me, other)? != "friend" {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "not_friends", "Переписка — только с друзьями"));
    }
    Ok(())
}

/// Собеседники: все друзья, сверху — с последними сообщениями; число непрочитанных.
pub async fn chats(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    adult(&v)?;
    let me = v.profile.id;
    let items: Vec<Value> = app.db.with(|c| {
        let mut st = c.prepare(&format!(
            "SELECT {PERSON_COLS},
                    (SELECT body FROM messages m WHERE (m.from_id = p.id AND m.to_id = ?1) OR (m.from_id = ?1 AND m.to_id = p.id) ORDER BY m.id DESC LIMIT 1),
                    (SELECT MAX(created_at) FROM messages m WHERE (m.from_id = p.id AND m.to_id = ?1) OR (m.from_id = ?1 AND m.to_id = p.id)) AS last_at,
                    (SELECT COUNT(*) FROM messages m WHERE m.from_id = p.id AND m.to_id = ?1 AND m.read_at IS NULL)
             FROM friends f JOIN profiles p ON p.id = CASE WHEN f.a = ?1 THEN f.b ELSE f.a END
             WHERE (f.a = ?1 OR f.b = ?1) AND f.status = 'accepted' AND {SOCIAL}
             ORDER BY last_at IS NULL, last_at DESC, p.name COLLATE NOCASE"
        ))?;
        let r = st.query_map([me], |r| {
            Ok(json!({"person": person(r, 0)?, "last": r.get::<_, Option<String>>(4)?,
                      "at": r.get::<_, Option<i64>>(5)?, "unread": r.get::<_, i64>(6)?}))
        })?;
        r.collect()
    })?;
    Ok(Json(json!({"items": items})))
}

#[derive(Deserialize)]
pub struct ChatQuery {
    /// Только сообщения новее этого id (опрос раз в несколько секунд).
    after: Option<i64>,
    /// Более старые, чем этот id (подгрузка вверх).
    before: Option<i64>,
}

pub async fn chat(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Query(q): Query<ChatQuery>,
) -> ApiResult<Json<Value>> {
    adult(&v)?;
    let me = v.profile.id;
    let who = social_profile(&app, id)?;
    friends_only(&app, me, id)?;
    let after = q.after.unwrap_or(0);
    let before = q.before.unwrap_or(i64::MAX);
    let mut items: Vec<Message> = app.db.with(|c| {
        let mut st = c.prepare(&format!(
            "SELECT m.id, m.from_id, m.body, m.created_at, m.read_at, m.title_id
             FROM messages m
             WHERE ((m.from_id = ?1 AND m.to_id = ?2) OR (m.from_id = ?2 AND m.to_id = ?1)) AND m.id > ?3 AND m.id < ?4
             ORDER BY m.id DESC LIMIT {PAGE}"
        ))?;
        let rows: Vec<(i64, i64, String, i64, Option<i64>, Option<i64>)> = st
            .query_map(params![me, id, after, before], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut tst = c.prepare(&format!("SELECT {CARD_COLS} FROM titles t WHERE t.id = ? AND {}", visible(false)))?;
        rows.into_iter()
            .map(|(mid, from, body, at, read, title)| {
                let title = match title {
                    Some(t) => tst.query_row([t], |r| card_at(r, 0)).optional()?,
                    None => None,
                };
                Ok(Message { id: mid, mine: from == me, body, created_at: at, read: read.is_some(), title })
            })
            .collect()
    })?;
    items.reverse();
    app.db.with(|c| {
        c.execute(
            "UPDATE messages SET read_at = ? WHERE to_id = ? AND from_id = ? AND read_at IS NULL",
            params![now_ms(), me, id],
        )
    })?;
    Ok(Json(json!({"with": who, "items": items})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendBody {
    #[serde(default)]
    body: String,
    title_id: Option<i64>,
}

pub async fn send(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    Json(b): Json<SendBody>,
) -> ApiResult<Json<Message>> {
    adult(&v)?;
    let me = v.profile.id;
    social_profile(&app, id)?;
    friends_only(&app, me, id)?;
    let body = text(&b.body, MAX_MESSAGE, "Сообщение")?;
    if let Some(t) = b.title_id {
        title_visible(&app, &v, t)?;
    }
    if body.is_empty() && b.title_id.is_none() {
        return Err(ApiError::bad("empty", "Пустое сообщение"));
    }
    if !app.message_limit.blocked(&me.to_string()) {
        app.message_limit.fail(&me.to_string());
    } else {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Слишком часто — подождите минуту"));
    }
    let at = now_ms();
    let (mid, title) = app.db.with(|c| {
        c.execute(
            "INSERT INTO messages (from_id, to_id, body, title_id, created_at) VALUES (?, ?, ?, ?, ?)",
            params![me, id, body, b.title_id, at],
        )?;
        let mid = c.last_insert_rowid();
        let title = match b.title_id {
            Some(t) => c
                .query_row(&format!("SELECT {CARD_COLS} FROM titles t WHERE t.id = ?"), [t], |r| card_at(r, 0))
                .optional()?,
            None => None,
        };
        Ok((mid, title))
    })?;
    Ok(Json(Message { id: mid, mine: true, body, created_at: at, read: false, title }))
}

/// Для значков в шапке: просьбы дружбы и непрочитанные сообщения.
pub async fn badges(State(app): State<Arc<App>>, v: Viewer) -> ApiResult<Json<Value>> {
    if v.profile.kids {
        return Ok(Json(json!({"requests": 0, "unread": 0})));
    }
    let me = v.profile.id;
    let (requests, unread): (i64, i64) = app.db.with(|c| {
        c.query_row(
            "SELECT (SELECT COUNT(*) FROM friends WHERE b = ?1 AND status = 'pending'),
                    (SELECT COUNT(*) FROM messages m WHERE m.to_id = ?1 AND m.read_at IS NULL
                       AND EXISTS (SELECT 1 FROM friends f WHERE f.status = 'accepted'
                         AND ((f.a = m.from_id AND f.b = ?1) OR (f.b = m.from_id AND f.a = ?1))))",
            [me],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
    })?;
    Ok(Json(json!({"requests": requests, "unread": unread})))
}

// ---------- фото профиля ----------

pub async fn set_photo(
    State(app): State<Arc<App>>,
    v: Viewer,
    Path(id): Path<i64>,
    body: axum::body::Bytes,
) -> ApiResult<Json<Value>> {
    super::profiles::own(&app, &v, id)?;
    if body.len() > crate::images::MAX_IMAGE {
        return Err(ApiError::bad("too_big", "Картинка больше 15 МБ"));
    }
    if crate::images::sniff(&body).is_none() {
        return Err(ApiError::bad("bad_image", "Нужна картинка: JPEG, PNG или WebP"));
    }
    // Квадрат 256 px: аватар грузится мгновенно. Без ffmpeg — как есть.
    let ffmpeg = app.tools.read().ffmpeg.clone();
    let a = app.clone();
    let raw = body.to_vec();
    let name = tokio::task::spawn_blocking(move || a.images.save(&raw))
        .await
        .map_err(ApiError::internal)?
        .map_err(|e| ApiError::bad("bad_image", e.to_string()))?;
    let name = match (ffmpeg, app.images.path(&name)) {
        (Some(ff), Some(src)) => match crate::library::probe::square_image(&ff, &src, 256).await {
            Ok(b) => app.images.save(&b).unwrap_or(name),
            Err(_) => name,
        },
        _ => name,
    };
    app.db.with(|c| c.execute("UPDATE profiles SET photo = ? WHERE id = ?", params![name, id]))?;
    Ok(Json(json!({"photo": format!("/img/{name}")})))
}

pub async fn delete_photo(State(app): State<Arc<App>>, v: Viewer, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    super::profiles::own(&app, &v, id)?;
    app.db.with(|c| c.execute("UPDATE profiles SET photo = NULL WHERE id = ?", [id]))?;
    Ok(Json(json!({"ok": true})))
}
