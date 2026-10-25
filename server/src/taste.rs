//! Вкус профиля (как у Netflix): что смотрел и как оценил → веса жанров → «совпадение» в
//! процентах, подборки, без «не для меня» в рядах.

use crate::api::Card;
use crate::App;
use rusqlite::params;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct Taste {
    genres: HashMap<String, f64>,
    pub ratings: HashMap<i64, i32>,
    pub list: HashSet<i64>,
    /// Что начинал смотреть (title_id → досмотрел хотя бы одну серию или фильм целиком).
    pub watched: HashMap<i64, bool>,
    /// Последнее просмотренное - для ряда «Потому что вы смотрели».
    pub recent: Vec<i64>,
    /// Отмечено «Просмотрено» (title_id → когда).
    pub seen: HashMap<i64, i64>,
    /// «Смотрю», отмеченное вручную (без начатых файлов).
    pub manual: HashSet<i64>,
    /// «Больше не смотрю» (title_id → когда); начал снова позже - не считается.
    pub dropped: HashMap<i64, i64>,
    /// Когда последний раз смотрел (title_id → время прогресса).
    pub last: HashMap<i64, i64>,
}

impl Taste {
    /// Только отметки и статусы, без весов жанров (быстро: для друзей и ленты «смотрят друзья»).
    pub fn marks(app: &App, profile: i64) -> rusqlite::Result<Taste> {
        app.db.with(|c| Self::read_marks(c, profile))
    }

    pub fn load(app: &App, profile: i64) -> rusqlite::Result<Taste> {
        app.db.with(|c| {
            let mut t = Self::read_marks(c, profile)?;
            // Веса жанров: досмотрел - 2, начал - 1, в списке - 1; оценка заменяет всё: 3, 2, -3.
            let mut weight: HashMap<i64, f64> = HashMap::new();
            for (id, done) in &t.watched {
                weight.insert(*id, if *done { 2.0 } else { 1.0 });
            }
            for id in t.seen.keys() {
                weight.insert(*id, 2.0);
            }
            for id in &t.list {
                *weight.entry(*id).or_insert(0.0) += 1.0;
            }
            for (id, v) in &t.ratings {
                weight.insert(
                    *id,
                    match v {
                        2 => 3.0,
                        1 => 2.0,
                        _ => -3.0,
                    },
                );
            }
            let mut st = c.prepare("SELECT genres FROM titles WHERE id = ?")?;
            for (id, w) in weight {
                let g: Option<String> = st.query_row(params![id], |r| r.get(0)).ok();
                for genre in crate::api::json_list(g) {
                    *t.genres.entry(genre).or_insert(0.0) += w;
                }
            }
            Ok(t)
        })
    }

    fn read_marks(c: &rusqlite::Connection, profile: i64) -> rusqlite::Result<Taste> {
        {
            let mut t = Taste::default();
            let mut st = c.prepare(
                "SELECT f.title_id, MAX(p.done), MAX(p.updated_at) AS at FROM progress p JOIN files f ON f.id = p.file_id
                 WHERE p.profile_id = ? GROUP BY f.title_id ORDER BY at DESC",
            )?;
            for r in
                st.query_map([profile], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?, r.get::<_, i64>(2)?)))?
            {
                let (id, done, at) = r?;
                t.recent.push(id);
                t.watched.insert(id, done);
                t.last.insert(id, at);
            }
            let mut st = c.prepare("SELECT title_id, at FROM dropped WHERE profile_id = ?")?;
            for r in st.query_map([profile], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
                let (id, at) = r?;
                t.dropped.insert(id, at);
            }
            let mut st = c.prepare("SELECT title_id FROM watching WHERE profile_id = ?")?;
            for r in st.query_map([profile], |r| r.get::<_, i64>(0))? {
                t.manual.insert(r?);
            }
            // Брошенное не висит в «Продолжить» и не подсказывает «Потому что вы смотрели».
            let dropped: Vec<i64> = t.recent.iter().copied().filter(|id| t.is_dropped(*id)).collect();
            t.recent.retain(|id| !dropped.contains(id));
            let mut st = c.prepare("SELECT title_id, value FROM ratings WHERE profile_id = ?")?;
            for r in st.query_map([profile], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i32>(1)?)))? {
                let (id, v) = r?;
                t.ratings.insert(id, v);
            }
            let mut st = c.prepare("SELECT title_id, at FROM watched WHERE profile_id = ?")?;
            for r in st.query_map([profile], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
                let (id, at) = r?;
                t.seen.insert(id, at);
            }
            let mut st = c.prepare("SELECT title_id FROM my_list WHERE profile_id = ?")?;
            for r in st.query_map([profile], |r| r.get::<_, i64>(0))? {
                t.list.insert(r?);
            }
            Ok(t)
        }
    }

    /// Мало истории - процентов не показываем, как и Netflix для нового профиля.
    pub fn known(&self) -> bool {
        self.genres.values().filter(|w| **w > 0.0).sum::<f64>() >= 2.0
    }

    /// 55-99 %, только если вкус известен; ниже 70 % не показываем.
    pub fn match_pct(&self, genres: &[String]) -> Option<i32> {
        if !self.known() || genres.is_empty() {
            return None;
        }
        let max = self.genres.values().cloned().fold(0.0_f64, f64::max);
        if max <= 0.0 {
            return None;
        }
        let mut a: Vec<f64> =
            genres.iter().map(|g| (self.genres.get(g).copied().unwrap_or(0.0) / max).clamp(-1.0, 1.0)).collect();
        a.sort_by(|x, y| y.partial_cmp(x).unwrap_or(std::cmp::Ordering::Equal));
        let top: Vec<f64> = a.into_iter().take(2).collect();
        let s = top.iter().sum::<f64>() / top.len() as f64;
        let pct = (55.0 + 44.0 * s.clamp(0.0, 1.0)).round() as i32;
        (pct >= 70).then_some(pct)
    }

    pub fn disliked(&self, id: i64) -> bool {
        self.ratings.get(&id) == Some(&-1)
    }

    pub fn apply(&self, cards: &mut [Card]) {
        for c in cards {
            c.in_list = self.list.contains(&c.id);
            c.my_rating = self.ratings.get(&c.id).copied();
            c.match_pct = self.match_pct(&c.genres);
            c.watched_at = self.seen.get(&c.id).copied();
            c.status = self.status(c.id);
        }
    }

    /// «Больше не смотрю» и с тех пор не начинал.
    pub fn is_dropped(&self, id: i64) -> bool {
        self.dropped.get(&id).is_some_and(|at| self.last.get(&id).is_none_or(|l| l <= at))
    }

    /// Просмотрено → смотрю (есть прогресс и не бросил, или отметил сам) → хочу посмотреть.
    pub fn status(&self, id: i64) -> Option<&'static str> {
        if self.seen.contains_key(&id) {
            Some("watched")
        } else if self.manual.contains(&id) || (self.watched.contains_key(&id) && !self.is_dropped(id)) {
            Some("watching")
        } else if self.list.contains(&id) {
            Some("want")
        } else if self.is_dropped(id) {
            Some("dropped")
        } else {
            None
        }
    }
}

/// Возрастная граница из рейтинга: «16+», «PG-13», «TV-MA», «R»… → лет.
pub fn age_min(age: &str) -> Option<i32> {
    let a = age.trim().to_uppercase();
    if let Some(n) = a.strip_suffix('+').and_then(|n| n.trim().parse::<i32>().ok()) {
        return Some(n);
    }
    Some(match a.as_str() {
        "G" | "TV-Y" | "TV-G" | "U" | "0" => 0,
        "TV-Y7" | "TV-Y7-FV" => 7,
        "PG" | "TV-PG" => 10,
        "PG-13" => 13,
        "TV-14" => 14,
        "R" | "TV-MA" | "M" => 17,
        "NC-17" | "X" => 18,
        _ => return a.parse::<i32>().ok().filter(|n| (0..=21).contains(n)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages() {
        assert_eq!(age_min("16+"), Some(16));
        assert_eq!(age_min("PG-13"), Some(13));
        assert_eq!(age_min("tv-ma"), Some(17));
        assert_eq!(age_min("6"), Some(6));
        assert_eq!(age_min("Unrated"), None);
    }

    #[test]
    fn status_priority() {
        let mut t = Taste::default();
        t.list.extend([1, 2, 3]);
        t.watched.insert(2, false);
        t.watched.insert(3, true);
        t.seen.insert(3, 5);
        assert_eq!(t.status(1), Some("want"));
        assert_eq!(t.status(2), Some("watching"));
        assert_eq!(t.status(3), Some("watched"));
        assert_eq!(t.status(4), None);
        // Бросил - не «Смотрю», пока не начал снова; отметил сам - «Смотрю» и без прогресса.
        t.last.insert(2, 10);
        t.dropped.insert(2, 20);
        assert_eq!(t.status(2), Some("want"));
        t.last.insert(2, 30);
        assert_eq!(t.status(2), Some("watching"));
        t.manual.insert(4);
        assert_eq!(t.status(4), Some("watching"));
    }

    #[test]
    fn match_follows_genres() {
        let mut t = Taste::default();
        t.genres.insert("Драма".into(), 6.0);
        t.genres.insert("Комедия".into(), 2.0);
        t.genres.insert("Ужасы".into(), -3.0);
        assert_eq!(t.match_pct(&["Драма".into()]), Some(99));
        assert!(t.match_pct(&["Ужасы".into()]).is_none());
        let mixed = t.match_pct(&["Драма".into(), "Комедия".into()]).unwrap();
        assert!((70..99).contains(&mixed));
        assert!(Taste::default().match_pct(&["Драма".into()]).is_none());
    }
}
