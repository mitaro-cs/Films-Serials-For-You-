//! SQLite: одно соединение под мьютексом (запросы короткие), WAL, внешние ключи.
//! Миграции — по `PRAGMA user_version`, старые шаги не редактировать — только добавлять новые.

use anyhow::Context;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

const MIGRATIONS: &[&str] = &[
    // 1 — первая схема
    r#"
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE libraries (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL DEFAULT 'auto' CHECK (kind IN ('auto','movies','series')),
  added_at INTEGER NOT NULL
);

CREATE TABLE titles (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('movie','series')),
  key TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  original_title TEXT,
  year INTEGER,
  overview TEXT,
  tagline TEXT,
  genres TEXT NOT NULL DEFAULT '[]',
  countries TEXT NOT NULL DEFAULT '[]',
  people TEXT NOT NULL DEFAULT '[]',
  languages TEXT NOT NULL DEFAULT '[]',
  aka TEXT NOT NULL DEFAULT '[]',
  gallery TEXT NOT NULL DEFAULT '[]',
  rating REAL,
  votes INTEGER,
  runtime INTEGER,
  release_date TEXT,
  imdb_id TEXT,
  age TEXT,
  poster TEXT,
  backdrop TEXT,
  logo TEXT,
  provider TEXT,
  provider_id TEXT,
  meta_status TEXT NOT NULL DEFAULT 'pending' CHECK (meta_status IN ('pending','matched','none','manual')),
  locked INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0,
  added_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE files (
  id INTEGER PRIMARY KEY,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  library_id INTEGER REFERENCES libraries(id) ON DELETE CASCADE,
  path TEXT NOT NULL UNIQUE,
  size INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  season INTEGER,
  episode INTEGER,
  episode_end INTEGER,
  name TEXT,
  overview TEXT,
  still TEXT,
  air_date TEXT,
  duration REAL,
  probe TEXT,
  added_at INTEGER NOT NULL,
  missing INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX files_title ON files(title_id, season, episode);

CREATE TABLE users (
  id INTEGER PRIMARY KEY,
  login TEXT NOT NULL UNIQUE COLLATE NOCASE,
  name TEXT NOT NULL,
  pass TEXT,
  role TEXT NOT NULL CHECK (role IN ('host','admin','viewer')),
  can_download INTEGER NOT NULL DEFAULT 1,
  disabled INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  seen_at INTEGER
);

CREATE TABLE sessions (
  hash TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  local INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  seen_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE TABLE invites (
  code TEXT PRIMARY KEY,
  note TEXT,
  uses_left INTEGER,
  expires_at INTEGER,
  used INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);

CREATE TABLE progress (
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  position REAL NOT NULL,
  duration REAL NOT NULL,
  done INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, file_id)
);
CREATE INDEX progress_recent ON progress(user_id, updated_at DESC);

CREATE TABLE my_list (
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  added_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, title_id)
);
"#,
    // 2 — профили («Кто смотрит?»), оценки, отметки заставки и титров, превью перемотки
    r#"
CREATE TABLE profiles (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  avatar INTEGER NOT NULL DEFAULT 0,
  kids INTEGER NOT NULL DEFAULT 0,
  autoplay INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL
);
CREATE INDEX profiles_user ON profiles(user_id);
INSERT INTO profiles (user_id, name, avatar, created_at) SELECT id, name, id % 12, created_at FROM users;

CREATE TABLE progress2 (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  position REAL NOT NULL,
  duration REAL NOT NULL,
  done INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, file_id)
);
INSERT INTO progress2 SELECT pr.id, p.file_id, p.position, p.duration, p.done, p.updated_at
  FROM progress p JOIN profiles pr ON pr.user_id = p.user_id;
DROP TABLE progress;
ALTER TABLE progress2 RENAME TO progress;
CREATE INDEX progress_recent ON progress(profile_id, updated_at DESC);
CREATE INDEX progress_when ON progress(updated_at);

CREATE TABLE my_list2 (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  added_at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, title_id)
);
INSERT INTO my_list2 SELECT pr.id, l.title_id, l.added_at FROM my_list l JOIN profiles pr ON pr.user_id = l.user_id;
DROP TABLE my_list;
ALTER TABLE my_list2 RENAME TO my_list;

CREATE TABLE ratings (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  value INTEGER NOT NULL CHECK (value IN (-1, 1, 2)),
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, title_id)
);

-- Заставка и титры, отмеченные хостом: на весь сезон (у фильма season = 0).
CREATE TABLE markers (
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  season INTEGER NOT NULL,
  intro_start REAL,
  intro_end REAL,
  credits REAL,
  PRIMARY KEY (title_id, season)
);

ALTER TABLE sessions ADD COLUMN profile_id INTEGER REFERENCES profiles(id) ON DELETE SET NULL;
ALTER TABLE titles ADD COLUMN poster_sm TEXT;
ALTER TABLE titles ADD COLUMN age_min INTEGER;
ALTER TABLE files ADD COLUMN trickplay TEXT;
-- Главы (заставка, титры) — заново через ffprobe.
UPDATE files SET probe = NULL;
"#,
    // 3 — «Просмотрено» (билет с датой) и цвет карточки по постеру
    r#"
CREATE TABLE watched (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, title_id)
);
-- Досмотренные фильмы — сразу «Просмотрено».
INSERT OR IGNORE INTO watched (profile_id, title_id, at)
  SELECT p.profile_id, f.title_id, MAX(p.updated_at) FROM progress p
  JOIN files f ON f.id = p.file_id JOIN titles t ON t.id = f.title_id
  WHERE p.done = 1 AND t.kind = 'movie' GROUP BY p.profile_id, f.title_id;
ALTER TABLE titles ADD COLUMN accent TEXT;
-- Лёгкие постеры — заново, заодно посчитается цвет.
UPDATE titles SET poster_sm = NULL;
"#,
];

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("открыть базу {}", path.display()))?;
        Self::init(conn)
    }

    pub fn memory() -> anyhow::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> anyhow::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;",
        )?;
        let version: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > MIGRATIONS.len() {
            anyhow::bail!("база от более новой версии программы — обновите приложение");
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql).with_context(|| format!("миграция {}", i + 1))?;
            tx.pragma_update(None, "user_version", i + 1)?;
            tx.commit()?;
        }
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    /// Короткая работа с базой. Долгие операции (скан) делают много коротких вызовов.
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> rusqlite::Result<T> {
        f(&self.0.lock())
    }

    pub fn tx<T>(&self, f: impl FnOnce(&rusqlite::Transaction) -> rusqlite::Result<T>) -> rusqlite::Result<T> {
        let mut c = self.0.lock();
        let tx = c.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        self.with(|c| c.query_row("SELECT value FROM settings WHERE key = ?", [key], |r| r.get(0)).optional())
            .ok()
            .flatten()
    }

    pub fn setting_or(&self, key: &str, default: &str) -> String {
        self.setting(key).filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_string())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO settings(key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(|_| ())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_and_keeps_settings() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.db");
        let db = Db::open(&p).unwrap();
        db.set_setting("a", "1").unwrap();
        db.set_setting("a", "2").unwrap();
        drop(db);
        let db = Db::open(&p).unwrap();
        assert_eq!(db.setting("a").as_deref(), Some("2"));
        assert_eq!(db.setting_or("missing", "x"), "x");
    }

    #[test]
    fn watched_movies_become_tickets() {
        let db = Db::memory().unwrap();
        db.with(|c| {
            c.execute_batch(
                "INSERT INTO users (id, login, name, role, created_at) VALUES (1, 'a', 'A', 'host', 0);
                 INSERT INTO profiles (id, user_id, name, created_at) VALUES (1, 1, 'A', 0);
                 INSERT INTO titles (id, kind, key, title, added_at, updated_at) VALUES (1, 'movie', 'm', 'M', 0, 0);
                 INSERT INTO files (id, title_id, path, size, mtime, added_at) VALUES (1, 1, '/m.mkv', 1, 0, 0);",
            )
        })
        .unwrap();
        // Миграция 3 на уже открытой базе не повторяется — проверяем её запрос напрямую.
        let sql = MIGRATIONS[2]
            .split("ALTER TABLE")
            .next()
            .unwrap()
            .replace("CREATE TABLE watched", "CREATE TABLE IF NOT EXISTS watched");
        db.with(|c| {
            c.execute("INSERT INTO progress VALUES (1, 1, 100, 100, 1, 42)", [])?;
            c.execute("DELETE FROM watched", [])?;
            c.execute_batch(&sql)?;
            c.query_row("SELECT at FROM watched WHERE profile_id = 1 AND title_id = 1", [], |r| r.get::<_, i64>(0))
        })
        .map(|at| assert_eq!(at, 42))
        .unwrap();
    }
}
