//! Тема недели на главной (alpha 0.5): «Неделя Нолана», «Неделя ужасов», «Советская классика».
//! Фильмы темы - с IMDb (поиск по жанру и стране или фильмография режиссёра), у себя - отмеченные
//! «Есть у нас». Хост выбирает тему в «Управлении» или свою (название + жанр или режиссёр); без
//! выбора тема меняется сама каждую неделю, а к Хэллоуину и Новому году - своя.

use crate::discover::{self, Src, MOVIE};
use crate::util::now_ms;
use crate::App;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Откуда фильмы темы.
pub enum From {
    /// Жанр IMDb (`Sci-Fi`, `Horror`).
    Genre(&'static str),
    /// Режиссёр - id IMDb (`nm…`): его фильмы.
    Director(&'static str),
    /// Страны производства (у IMDb СССР - `SUHH`, не `SU`).
    Countries(&'static [&'static str]),
}

pub struct Theme {
    pub key: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub from: From,
}

const fn t(key: &'static str, title: &'static str, subtitle: &'static str, from: From) -> Theme {
    Theme { key, title, subtitle, from }
}

pub const THEMES: &[Theme] = &[
    t("nolan", "Неделя Нолана", "Время, сны и головоломки", From::Director("nm0634240")),
    t("scifi", "Неделя фантастики", "Космос, будущее и другие миры", From::Genre("Sci-Fi")),
    t("tarantino", "Неделя Тарантино", "Диалоги, музыка и месть", From::Director("nm0000233")),
    t("comedy", "Неделя комедий", "Чтобы смеяться всей компанией", From::Genre("Comedy")),
    t("miyazaki", "Неделя Миядзаки", "Ручная анимация студии Ghibli", From::Director("nm0594503")),
    t("mystery", "Неделя детективов", "Кто это сделал?", From::Genre("Mystery")),
    t("fincher", "Неделя Финчера", "Мрачные тайны и одержимость", From::Director("nm0000399")),
    t("adventure", "Неделя приключений", "В дорогу - не вставая с дивана", From::Genre("Adventure")),
    t("soviet", "Советская классика", "Фильмы, которые знают все", From::Countries(&["SUHH"])),
    t("villeneuve", "Неделя Вильнёва", "Тишина, масштаб и пустыня", From::Director("nm0898288")),
    t("war", "Неделя военного кино", "Чтобы помнить", From::Genre("War")),
    t("romance", "Неделя мелодрам", "Для вечера вдвоём", From::Genre("Romance")),
    t("kubrick", "Неделя Кубрика", "Каждый кадр - картина", From::Director("nm0000040")),
    t("thriller", "Неделя триллеров", "Не отрываясь до титров", From::Genre("Thriller")),
    t("horror", "Неделя ужасов", "К Хэллоуину - со светом", From::Genre("Horror")),
    t("family", "Новогодние каникулы", "Семейное кино на праздники", From::Genre("Family")),
];

/// Тема по дате (мс Unix): последняя неделя октября - ужасы, с 24 декабря по 8 января - семейное,
/// иначе по номеру недели по кругу (без этих двух).
pub fn auto(ms: i64) -> &'static Theme {
    let ymd = discover::ymd(ms);
    let (m, d): (u32, u32) = (ymd[5..7].parse().unwrap_or(1), ymd[8..10].parse().unwrap_or(1));
    let key = if m == 10 && d >= 25 {
        "horror"
    } else if (m == 12 && d >= 24) || (m == 1 && d <= 8) {
        "family"
    } else {
        let regular: Vec<&Theme> = THEMES.iter().filter(|t| t.key != "horror" && t.key != "family").collect();
        // Недели - с понедельника: 1 января 1970 - четверг.
        let week = (ms.div_euclid(86_400_000) + 3).div_euclid(7) as usize;
        return regular[week % regular.len()];
    };
    THEMES.iter().find(|t| t.key == key).expect("тема есть в списке")
}

/// Выбор хоста (`settings theme.choice`): авто, одна из тем или своя.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    /// "auto", ключ темы или "custom".
    pub mode: String,
    #[serde(default)]
    pub title: String,
    /// Своя тема: жанр по-русски («Фантастика») или имя режиссёра.
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub director: String,
}

pub fn choice(app: &App) -> Choice {
    app.db
        .setting("theme.choice")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Choice { mode: "auto".into(), ..Default::default() })
}

/// Фильмы режиссёра (`nm…`): фильмография с IMDb (раз в неделю), только фильмы, известные - первыми.
async fn director_ids(app: &Arc<App>, id: &str) -> anyhow::Result<Vec<String>> {
    tokio::time::timeout(Duration::from_secs(10), crate::directors::fetch(app, id, "director")).await??;
    let works: Option<String> = app.db.with(|c| {
        c.query_row("SELECT works FROM ext_people WHERE id = ? AND role = 'director'", [id], |r| r.get(0)).optional()
    })?;
    let ids: Vec<String> = works.and_then(|w| serde_json::from_str(&w).ok()).unwrap_or_default();
    let list = serde_json::to_string(&ids)?;
    let mut rows: Vec<(String, i64)> = app.db.with(|c| {
        let mut st = c.prepare(
            "SELECT e.id, COALESCE(e.votes, 0) FROM ext_titles e JOIN json_each(?1) j ON j.value = e.id
             WHERE e.kind = 'movie' AND (e.year IS NULL OR e.year <= CAST(strftime('%Y','now') AS INTEGER))",
        )?;
        let r = st.query_map([&list], |r| Ok((r.get(0)?, r.get(1)?)))?;
        r.collect()
    })?;
    rows.sort_by_key(|r| std::cmp::Reverse(r.1));
    Ok(rows.into_iter().map(|r| r.0).take(24).collect())
}

/// Тема сейчас: подпись и ids фильмов ленты (до 24).
pub struct Current {
    pub key: String,
    pub title: String,
    pub subtitle: String,
    pub ids: Vec<String>,
}

pub async fn current(app: &Arc<App>) -> anyhow::Result<Current> {
    let ch = choice(app);
    if ch.mode == "custom" && !ch.title.trim().is_empty() {
        let ids = if !ch.director.trim().is_empty() {
            match crate::directors::resolve(app, ch.director.trim()).await? {
                Some(id) => director_ids(app, &id).await?,
                None => vec![],
            }
        } else if let Some((genre, _, types)) = discover::genre_query(&ch.genre) {
            discover::genre_search(app, genre, types).await?
        } else {
            vec![]
        };
        return Ok(Current { key: "custom".into(), title: ch.title.trim().into(), subtitle: String::new(), ids });
    }
    let theme = THEMES.iter().find(|t| t.key == ch.mode).unwrap_or_else(|| auto(now_ms()));
    let ids = match &theme.from {
        From::Genre(g) => {
            let src = Src::Search { types: MOVIE, genre: Some(g), countries: &[], days: None, votes: 25_000 };
            discover::search_cached(app, &format!("theme:{}", theme.key), &src).await?
        }
        From::Countries(c) => {
            let src = Src::Search { types: MOVIE, genre: None, countries: c, days: None, votes: 3_000 };
            discover::search_cached(app, &format!("theme:{}", theme.key), &src).await?
        }
        From::Director(id) => director_ids(app, id).await?,
    };
    Ok(Current {
        key: theme.key.into(),
        title: theme.title.into(),
        subtitle: theme.subtitle.into(),
        ids: ids.into_iter().take(24).collect(),
    })
}

/// Список тем для «Управления».
pub fn list() -> Value {
    Value::Array(
        THEMES.iter().map(|t| serde_json::json!({ "key": t.key, "title": t.title, "subtitle": t.subtitle })).collect(),
    )
}

pub fn save(app: &App, ch: &Choice) -> rusqlite::Result<()> {
    app.db.set_setting("theme.choice", &serde_json::to_string(ch).unwrap_or_default())?;
    // Ряд пересоберётся при следующем показе.
    app.db.with(|c| c.execute("DELETE FROM catalog_search_cache WHERE query LIKE 'theme:%'", params![]))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(y: i32, m: u32, d: u32) -> i64 {
        // Дни от 1970-01-01 по алгоритму Хиннанта (обратный к discover::ymd).
        let y = if m <= 2 { y - 1 } else { y } as i64;
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m as i64 + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        (era * 146_097 + doe - 719_468) * 86_400_000 + 12 * 3_600_000
    }

    #[test]
    fn auto_theme_by_date() {
        assert_eq!(discover::ymd(ms(2026, 10, 28)), "2026-10-28");
        assert_eq!(auto(ms(2026, 10, 28)).key, "horror", "Хэллоуин");
        assert_eq!(auto(ms(2026, 12, 31)).key, "family", "Новый год");
        assert_eq!(auto(ms(2027, 1, 5)).key, "family");
        let a = auto(ms(2026, 3, 2)).key;
        let b = auto(ms(2026, 3, 9)).key;
        assert_ne!(a, b, "каждую неделю - новая");
        assert_eq!(auto(ms(2026, 3, 3)).key, auto(ms(2026, 3, 4)).key, "внутри недели - одна");
        assert!(!["horror", "family"].contains(&a));
        assert_eq!(THEMES.iter().map(|t| t.key).collect::<std::collections::HashSet<_>>().len(), THEMES.len());
    }
}
