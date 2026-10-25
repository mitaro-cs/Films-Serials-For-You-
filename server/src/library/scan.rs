//! Обход папок библиотеки: новые файлы → записи, пропавшие → `missing`, вернувшиеся → обратно.
//! Ничего не удаляется из-за отключённого диска: прогресс просмотра переживает это.

use super::parse::{is_video, parse};
use crate::db::Db;
use crate::images::Images;
use crate::util::{norm, now_ms};
use rusqlite::{params, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ScanStats {
    pub files: usize,
    pub added: usize,
    pub missing: usize,
    pub titles: usize,
}

const SKIP_DIRS: &[&str] = &["@eaDir", "$RECYCLE.BIN", "System Volume Information", "#recycle", "lost+found"];

fn skip(name: &str) -> bool {
    name.starts_with('.') || SKIP_DIRS.iter().any(|d| d.eq_ignore_ascii_case(name))
}

fn is_sample(path: &Path, size: u64) -> bool {
    let n = path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
    size < 300 * 1024 * 1024 && (n.contains("sample") || n.contains("trailer"))
}

struct Existing {
    id: i64,
    size: i64,
    mtime: i64,
    missing: bool,
}

pub fn title_key(kind: &str, title: &str, year: Option<i32>) -> String {
    if kind == "series" {
        format!("series:{}", norm(title))
    } else {
        format!("movie:{}:{}", norm(title), year.map(|y| y.to_string()).unwrap_or_default())
    }
}

pub fn scan(db: &Db, images: &Images) -> anyhow::Result<ScanStats> {
    let libs: Vec<(i64, String, String)> = db.with(|c| {
        let mut st = c.prepare("SELECT id, path, kind FROM libraries ORDER BY id")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect()
    })?;
    let existing: HashMap<String, Existing> = db.with(|c| {
        let mut st = c.prepare("SELECT path, id, size, mtime, missing FROM files")?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Existing { id: r.get(1)?, size: r.get(2)?, mtime: r.get(3)?, missing: r.get(4)? },
            ))
        })?;
        rows.collect()
    })?;

    let mut stats = ScanStats::default();
    let mut seen: HashSet<String> = HashSet::new();
    for (lib_id, root, kind) in libs {
        let root = PathBuf::from(root);
        if !root.is_dir() {
            continue; // диск отключён: файлы этой папки станут `missing`
        }
        let force = match kind.as_str() {
            "movies" => Some(false),
            "series" => Some(true),
            _ => None,
        };
        let mut new_files: Vec<(PathBuf, u64, i64)> = Vec::new();
        // Папка, в которую ещё грузят файлы (`*.part`), — подождёт: иначе первая серия
        // успела бы стать «фильмом» до прихода остальных.
        let mut busy: HashMap<PathBuf, bool> = HashMap::new();
        let walker = walkdir::WalkDir::new(&root)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_name().to_str().map(skip).unwrap_or(true));
        for entry in walker.filter_map(Result::ok) {
            let path = entry.path();
            if !entry.file_type().is_file() || !is_video(path) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let size = meta.len();
            if let Some(dir) = path.parent() {
                let uploading = *busy.entry(dir.to_path_buf()).or_insert_with(|| has_part(dir));
                if uploading && !existing.contains_key(path.to_str().unwrap_or_default()) {
                    continue;
                }
            }
            if is_sample(path, size) {
                continue;
            }
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let Some(key) = path.to_str().map(String::from) else {
                continue;
            };
            stats.files += 1;
            if !seen.insert(key.clone()) {
                continue; // папка внутри другой библиотеки
            }
            match existing.get(&key) {
                Some(e) if e.missing || e.size != size as i64 || e.mtime != mtime => {
                    let changed = e.size != size as i64 || e.mtime != mtime;
                    db.with(|c| {
                        c.execute(
                            "UPDATE files SET missing = 0, size = ?, mtime = ?, probe = CASE WHEN ? THEN NULL ELSE probe END WHERE id = ?",
                            params![size as i64, mtime, changed, e.id],
                        )
                    })?;
                }
                Some(_) => {}
                None => new_files.push((path.to_path_buf(), size, mtime)),
            }
        }
        if !new_files.is_empty() {
            stats.added += new_files.len();
            add_files(db, images, lib_id, &root, force, new_files)?;
        }
    }

    let gone: Vec<i64> = existing.iter().filter(|(p, e)| !e.missing && !seen.contains(*p)).map(|(_, e)| e.id).collect();
    stats.missing = gone.len();
    db.tx(|tx| {
        for id in &gone {
            tx.execute("UPDATE files SET missing = 1 WHERE id = ?", [id])?;
        }
        tx.execute("DELETE FROM titles WHERE NOT EXISTS (SELECT 1 FROM files f WHERE f.title_id = titles.id)", [])?;
        Ok(())
    })?;
    stats.titles = db.with(|c| {
        c.query_row(
            "SELECT COUNT(*) FROM titles t WHERE EXISTS (SELECT 1 FROM files f WHERE f.title_id = t.id AND f.missing = 0)",
            [],
            |r| r.get::<_, i64>(0),
        )
    })? as usize;
    Ok(stats)
}

fn has_part(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .is_ok_and(|rd| rd.flatten().any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("part"))))
}

/// Папка сериала без папок сезонов: в ней не меньше двух видео с номером серии в имени.
fn numbered_dir(dir: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else { return false };
    rd.flatten()
        .map(|e| e.path())
        .filter(|p| is_video(p))
        .filter(|p| p.file_stem().and_then(|s| s.to_str()).is_some_and(super::parse::numbered))
        .take(2)
        .count()
        >= 2
}

fn add_files(
    db: &Db,
    images: &Images,
    lib_id: i64,
    root: &Path,
    force: Option<bool>,
    files: Vec<(PathBuf, u64, i64)>,
) -> anyhow::Result<()> {
    let now = now_ms();
    let mut art_dirs: Vec<(i64, PathBuf)> = Vec::new();
    let mut series_dirs: HashMap<PathBuf, bool> = HashMap::new();
    db.tx(|tx| {
        for (path, size, mtime) in &files {
            let rel = path.strip_prefix(root).unwrap_or(path);
            let force = match (force, path.parent()) {
                (None, Some(dir)) if dir != root => {
                    let yes = *series_dirs.entry(dir.to_path_buf()).or_insert_with(|| numbered_dir(dir));
                    yes.then_some(true)
                }
                _ => force,
            };
            let p = parse(rel, force);
            let kind = if p.is_episode() { "series" } else { "movie" };
            let key = title_key(kind, &p.title, p.year);
            let existing: Option<i64> =
                tx.query_row("SELECT id FROM titles WHERE key = ?", [&key], |r| r.get(0)).optional()?;
            let title_id = match existing {
                Some(id) => {
                    if p.year.is_some() {
                        tx.execute("UPDATE titles SET year = COALESCE(year, ?) WHERE id = ?", params![p.year, id])?;
                    }
                    id
                }
                None => {
                    tx.execute(
                        "INSERT INTO titles (kind, key, title, year, added_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
                        params![kind, key, p.title, p.year, now, now],
                    )?;
                    let id = tx.last_insert_rowid();
                    // Картинки рядом: у фильма — в его папке, у сериала — в папке сериала или сезона.
                    let mut dir = path.parent();
                    for _ in 0..2 {
                        match dir {
                            Some(d) if d != root && d.starts_with(root) => {
                                art_dirs.push((id, d.to_path_buf()));
                                if kind == "movie" {
                                    break;
                                }
                                dir = d.parent();
                            }
                            _ => break,
                        }
                    }
                    id
                }
            };
            tx.execute(
                "INSERT INTO files (title_id, library_id, path, size, mtime, season, episode, episode_end, added_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![title_id, lib_id, path.to_str(), *size as i64, mtime, p.season, p.episode, p.episode_end, now],
            )?;
        }
        Ok(())
    })?;
    for (id, dir) in art_dirs {
        let (poster, backdrop) = Images::local_art(&dir);
        let poster = poster.and_then(|p| images.import(&p).ok());
        let backdrop = backdrop.and_then(|p| images.import(&p).ok());
        if poster.is_some() || backdrop.is_some() {
            db.with(|c| {
                c.execute(
                    "UPDATE titles SET poster = COALESCE(poster, ?), backdrop = COALESCE(backdrop, ?) WHERE id = ?",
                    params![poster, backdrop, id],
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn lib(db: &Db, path: &Path, kind: &str) {
        db.with(|c| {
            c.execute("INSERT INTO libraries (path, kind, added_at) VALUES (?, ?, 0)", params![path.to_str(), kind])
        })
        .unwrap();
    }

    #[test]
    fn groups_episodes_and_tracks_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("media");
        fs::create_dir_all(root.join("Show (2020)/Season 1")).unwrap();
        fs::create_dir_all(root.join("Film (2010)")).unwrap();
        fs::write(root.join("Show (2020)/Season 1/Show.S01E01.mkv"), b"x").unwrap();
        fs::write(root.join("Show (2020)/Season 1/Show.S01E02.mkv"), b"x").unwrap();
        fs::write(root.join("Film (2010)/film.mp4"), b"x").unwrap();
        fs::write(root.join("Film (2010)/poster.jpg"), [0xFF, 0xD8, 0xFF, 0]).unwrap();
        fs::write(root.join("Film (2010)/notes.txt"), b"x").unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join(".hidden/x.mkv"), b"x").unwrap();

        let db = Db::memory().unwrap();
        let images = Images::new(dir.path().join("img")).unwrap();
        lib(&db, &root, "auto");
        let s = scan(&db, &images).unwrap();
        assert_eq!((s.files, s.added, s.titles), (3, 3, 2));

        let (kind, title, year, poster): (String, String, Option<i32>, Option<String>) = db
            .with(|c| {
                c.query_row("SELECT kind, title, year, poster FROM titles WHERE kind = 'movie'", [], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })
            })
            .unwrap();
        assert_eq!((kind.as_str(), title.as_str(), year), ("movie", "Film", Some(2010)));
        assert!(poster.is_some());

        fs::remove_file(root.join("Show (2020)/Season 1/Show.S01E02.mkv")).unwrap();
        let s = scan(&db, &images).unwrap();
        assert_eq!((s.added, s.missing, s.titles), (0, 1, 2));

        fs::write(root.join("Show (2020)/Season 1/Show.S01E02.mkv"), b"x").unwrap();
        scan(&db, &images).unwrap();
        let missing: i64 = db.with(|c| c.query_row("SELECT SUM(missing) FROM files", [], |r| r.get(0))).unwrap();
        assert_eq!(missing, 0);
    }

    #[test]
    fn numbered_folder_is_a_series_and_waits_for_upload() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("media");
        let show = root.join("Вольтрон");
        fs::create_dir_all(&show).unwrap();
        fs::write(show.join("01. The Rise of Voltron.mkv"), b"x").unwrap();
        fs::write(show.join("02. The Rise of Voltron Part 2.mkv"), b"x").unwrap();
        fs::write(show.join("03. Return of the Gladiator.mkv.part"), b"x").unwrap();
        fs::write(root.join("21 Jump Street.mkv"), b"x").unwrap();

        let db = Db::memory().unwrap();
        let images = Images::new(dir.path().join("img")).unwrap();
        lib(&db, &root, "auto");
        let s = scan(&db, &images).unwrap();
        assert_eq!(s.added, 1, "папку ещё грузят — ждём");

        fs::rename(show.join("03. Return of the Gladiator.mkv.part"), show.join("03. Return of the Gladiator.mkv"))
            .unwrap();
        scan(&db, &images).unwrap();
        let rows: Vec<(String, String, i64)> = db
            .with(|c| {
                let mut st = c.prepare(
                    "SELECT t.kind, t.title, COUNT(f.id) FROM titles t JOIN files f ON f.title_id = t.id GROUP BY t.id ORDER BY t.title",
                )?;
                let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                r.collect()
            })
            .unwrap();
        assert_eq!(rows, vec![("movie".into(), "21 Jump Street".into(), 1), ("series".into(), "Вольтрон".into(), 3)]);
    }
}
