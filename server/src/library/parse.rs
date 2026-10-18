//! Разбор имени файла и папок: название, год, сезон и серия.
//!
//! Понимает `Name.2010.1080p.mkv`, `Name (2010)/file.mkv`, `Show.S01E02.mkv`, `Show 1x02`,
//! `Show/Season 1/02.mkv`, «Сериал/1 сезон/Серия 2.mkv», `S01E01-E02`.

use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub title: String,
    pub year: Option<i32>,
    /// Есть у серий; у фильмов — `None`.
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub episode_end: Option<i32>,
}

impl Parsed {
    pub fn is_episode(&self) -> bool {
        self.episode.is_some()
    }
}

pub const VIDEO_EXT: &[&str] =
    &["mp4", "m4v", "mkv", "webm", "mov", "avi", "wmv", "ts", "m2ts", "mpg", "mpeg", "flv", "ogv", "3gp"];

pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXT.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

static SXXEYY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bs(\d{1,2})[ ._-]*e(\d{1,3})(?:[ ._-]*(?:-|e|е)[ ._-]*e?(\d{1,3}))?").unwrap());
static NXNN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(\d{1,2})x(\d{2,3})\b").unwrap());
static RU_SEASON_EP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:сезон|season)\s*(\d{1,2})\D{0,6}?(?:серия|эпизод|episode|ep)\s*(\d{1,3})|(\d{1,2})\s*сезон\D{0,6}?(\d{1,3})\s*серия").unwrap()
});
static EP_ONLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[ ._\-\[(])(?:e|ep|episode|серия|эпизод|выпуск)[ ._-]*(\d{1,3})(?:\b|$)|(?:^|[ ._\-\[(])(\d{1,3})[ ._-]*(?:серия|эпизод|выпуск)").unwrap()
});
static LEADING_NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d{1,3})(?:\D|$)").unwrap());
static SEASON_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:season|сезон|s)[ ._-]*(\d{1,2})\b|^\s*(\d{1,2})[ ._-]*(?:season|сезон)\b").unwrap()
});
static PLEX_DIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(.+?)\s*[(\[]((?:19|20)\d{2})[)\]]\s*$").unwrap());
static DIGITS4: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{4,}").unwrap());
static JUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[ .\[(_-])(?:2160p|1080p|1080i|720p|576p|480p|4k|uhd|hdr10?|dv|web[ .-]?dl|web[ .-]?rip|webrip|hdtv|bdrip|brrip|bluray|blu[ .-]?ray|dvdrip|dvd5|dvd9|hdrip|remux|x264|x265|h\.?264|h\.?265|hevc|avc|aac|ac3|dts|ddp?5\.1|5\.1|7\.1|mvo|dvo|avo|dub|itunes|proper|repack|extended|unrated|imax|rus|eng|ukr|sub|subs|lostfilm|newstudio|amedia|kinozal|rutracker)(?:$|[ .\])_-])",
    )
    .unwrap()
});

/// Текст вида `Name.Of.The.Movie` → `Name Of The Movie`, без хвостов в скобках.
fn tidy(s: &str) -> String {
    let s = s.replace(['.', '_'], " ");
    let s = s.trim_matches(|c: char| c.is_whitespace() || "-–—[(,:".contains(c));
    let mut out = String::new();
    for w in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    // Висящие скобки после обрезки по году: «Name (»
    out.trim_end_matches(|c: char| "([{-–—".contains(c) || c.is_whitespace()).to_string()
}

/// Название и год из строки: всё до года или до первого «технического» слова.
fn title_year(s: &str) -> (String, Option<i32>) {
    let mut cut = s.len();
    let mut year = None;
    let sep = |c: Option<char>| c.is_none_or(|c| " .[]()_-".contains(c));
    // Год в начале — это название («2012»), год берётся только после него.
    for m in DIGITS4.find_iter(s) {
        let n: i32 = m.as_str().parse().unwrap_or(0);
        if m.start() == 0 || m.len() != 4 || !(1900..=2099).contains(&n) {
            continue;
        }
        if sep(s[..m.start()].chars().next_back()) && sep(s[m.end()..].chars().next()) {
            year = Some(n);
            cut = m.start();
            break;
        }
    }
    if let Some(j) = JUNK.find(&s[..cut]) {
        if j.start() > 0 {
            cut = j.start();
        }
    }
    (tidy(&s[..cut]), year)
}

/// Имя похоже на серию без сезона: «01. Пилот», «Серия 5», «E07». Несколько таких в одной папке —
/// это сериал (`scan` решает по папке, по одному имени не угадать: «21 Jump Street»).
pub fn numbered(stem: &str) -> bool {
    (LEADING_NUM.is_match(stem) || EP_ONLY.is_match(stem)) && title_year(stem).1.is_none()
}

/// Разбор пути относительно корня библиотеки. `force`: Some(true) — сериал, Some(false) — фильм.
pub fn parse(rel: &Path, force: Option<bool>) -> Parsed {
    let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    let dirs: Vec<String> =
        rel.parent().map(|p| p.iter().filter_map(|c| c.to_str()).map(String::from).collect()).unwrap_or_default();

    // Папка сезона и папка сериала над ней.
    let mut season_dir: Option<(usize, i32)> = None;
    for (i, d) in dirs.iter().enumerate().rev() {
        if let Some(c) = SEASON_DIR.captures(d) {
            let n = c.get(1).or(c.get(2)).and_then(|m| m.as_str().parse().ok());
            if let Some(n) = n {
                season_dir = Some((i, n));
                break;
            }
        }
    }
    let show_dir = match season_dir {
        Some((i, _)) if i > 0 => Some(dirs[i - 1].clone()),
        Some(_) => None,
        None => dirs.last().cloned(),
    };

    let mut season = None;
    let mut episode = None;
    let mut episode_end = None;
    let mut before = stem.as_str();

    if force != Some(false) {
        if let Some(c) = SXXEYY.captures(&stem) {
            season = c[1].parse().ok();
            episode = c[2].parse().ok();
            episode_end = c.get(3).and_then(|m| m.as_str().parse().ok());
            before = &stem[..c.get(0).unwrap().start()];
        } else if let Some(c) = RU_SEASON_EP.captures(&stem) {
            season = c.get(1).or(c.get(3)).and_then(|m| m.as_str().parse().ok());
            episode = c.get(2).or(c.get(4)).and_then(|m| m.as_str().parse().ok());
            before = &stem[..c.get(0).unwrap().start()];
        } else if let Some(c) = NXNN.captures(&stem) {
            season = c[1].parse().ok();
            episode = c[2].parse().ok();
            before = &stem[..c.get(0).unwrap().start()];
        } else if season_dir.is_some() || force == Some(true) {
            if let Some(c) = EP_ONLY.captures(&stem) {
                episode = c.get(1).or(c.get(2)).and_then(|m| m.as_str().parse().ok());
                before = &stem[..c.get(0).unwrap().start()];
            } else if let Some(c) = LEADING_NUM.captures(&stem) {
                episode = c[1].parse().ok();
                before = "";
            }
            season = season_dir.map(|(_, n)| n).or(Some(1));
        }
    }

    if episode.is_some() {
        if season.is_none() {
            season = season_dir.map(|(_, n)| n).or(Some(1));
        }
        let (mut title, mut year) = title_year(before);
        if title.is_empty() {
            if let Some(d) = &show_dir {
                (title, year) = title_year(d);
            }
        }
        if title.is_empty() {
            title = tidy(&stem);
        }
        return Parsed { title, year, season, episode, episode_end };
    }

    // Фильм: имя файла, а если папка выглядит как «Название (год)» — она точнее.
    let (mut title, mut year) = title_year(&stem);
    if let Some(d) = dirs.last() {
        // «Название (год)» — так папки называют вручную (и Plex): ей верим больше, чем имени файла.
        if let Some(c) = PLEX_DIR.captures(d) {
            let y: i32 = c[2].parse().unwrap_or(0);
            if year.is_none_or(|fy| fy == y) {
                return Parsed { title: tidy(&c[1]), year: Some(y), season: None, episode: None, episode_end: None };
            }
        }
        let (dt, dy) = title_year(d);
        let (a, b) = (crate::util::norm(&dt), crate::util::norm(&title));
        let same = !a.is_empty() && (a.contains(&b) || b.contains(&a));
        if dy.is_some() && same && (year.is_none() || year == dy) {
            title = dt;
            year = dy;
        } else if title.is_empty() {
            title = dt;
            year = year.or(dy);
        }
    }
    if title.is_empty() {
        title = tidy(&stem);
    }
    Parsed { title, year, season: None, episode: None, episode_end: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Parsed {
        parse(Path::new(s), None)
    }

    #[test]
    fn movies() {
        let m = p("Inception.2010.1080p.BluRay.x264.mkv");
        assert_eq!((m.title.as_str(), m.year, m.episode), ("Inception", Some(2010), None));
        let m = p("Брат 2 (2000) BDRip.avi");
        assert_eq!((m.title.as_str(), m.year), ("Брат 2", Some(2000)));
        let m = p("Dune Part Two (2024)/dune.part.two.2160p.mkv");
        assert_eq!((m.title.as_str(), m.year), ("Dune Part Two", Some(2024)));
        let m = p("2012.2009.mkv");
        assert_eq!((m.title.as_str(), m.year), ("2012", Some(2009)));
        let m = p("Матрица.mp4");
        assert_eq!((m.title.as_str(), m.year), ("Матрица", None));
        let m = p("Interstellar.WEB-DL.mkv");
        assert_eq!(m.title, "Interstellar");
        let m = p("Начало (2010)/Inception.2010.1080p.mkv");
        assert_eq!((m.title.as_str(), m.year), ("Начало", Some(2010)));
        let m = p("Матрица [1999]/The.Matrix.mkv");
        assert_eq!((m.title.as_str(), m.year), ("Матрица", Some(1999)));
        let m = p("Фильмы 2023/Матрица.mp4");
        assert_eq!((m.title.as_str(), m.year), ("Матрица", None));
        let m = p("Movies/movie.mkv");
        assert_eq!(m.title, "movie");
    }

    #[test]
    fn episodes() {
        let e = p("The.Office.US.S02E05.720p.mkv");
        assert_eq!((e.title.as_str(), e.season, e.episode), ("The Office US", Some(2), Some(5)));
        let e = p("Friends/Season 3/Friends.S03E01-E02.mkv");
        assert_eq!((e.season, e.episode, e.episode_end), (Some(3), Some(1), Some(2)));
        let e = p("Breaking Bad (2008)/Season 1/01 - Pilot.mkv");
        assert_eq!((e.title.as_str(), e.year, e.season, e.episode), ("Breaking Bad", Some(2008), Some(1), Some(1)));
        let e = p("Кухня/2 сезон/Серия 7.mp4");
        assert_eq!((e.title.as_str(), e.season, e.episode), ("Кухня", Some(2), Some(7)));
        let e = p("Lost 1x04.avi");
        assert_eq!((e.title.as_str(), e.season, e.episode), ("Lost", Some(1), Some(4)));
        let e = p("Слово пацана/Слово пацана S01E03.mkv");
        assert_eq!((e.title.as_str(), e.episode), ("Слово пацана", Some(3)));
        let e = p("Сериал/Сезон 1 Серия 4.mkv");
        assert_eq!((e.title.as_str(), e.season, e.episode), ("Сериал", Some(1), Some(4)));
    }

    #[test]
    fn forced_kinds() {
        let e = parse(Path::new("Аниме/03.mkv"), Some(true));
        assert_eq!((e.title.as_str(), e.season, e.episode), ("Аниме", Some(1), Some(3)));
        let m = parse(Path::new("Show.S01E01.mkv"), Some(false));
        assert_eq!(m.episode, None);
    }

    #[test]
    fn video_extensions() {
        assert!(is_video(Path::new("a/B.MKV")));
        assert!(!is_video(Path::new("a/b.srt")));
    }
}
