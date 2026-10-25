//! SQLite: одно соединение под мьютексом (запросы короткие), WAL, внешние ключи.
//! Миграции - по `PRAGMA user_version`, старые шаги не редактировать - только добавлять новые.

use anyhow::Context;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

const MIGRATIONS: &[&str] = &[
    // 1 - первая схема
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
    // 2 - профили («Кто смотрит?»), оценки, отметки заставки и титров, превью перемотки
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
-- Главы (заставка, титры) - заново через ffprobe.
UPDATE files SET probe = NULL;
"#,
    // 3 - «Просмотрено» (билет с датой) и цвет карточки по постеру
    r#"
CREATE TABLE watched (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, title_id)
);
-- Досмотренные фильмы - сразу «Просмотрено».
INSERT OR IGNORE INTO watched (profile_id, title_id, at)
  SELECT p.profile_id, f.title_id, MAX(p.updated_at) FROM progress p
  JOIN files f ON f.id = p.file_id JOIN titles t ON t.id = f.title_id
  WHERE p.done = 1 AND t.kind = 'movie' GROUP BY p.profile_id, f.title_id;
ALTER TABLE titles ADD COLUMN accent TEXT;
-- Лёгкие постеры - заново, заодно посчитается цвет.
UPDATE titles SET poster_sm = NULL;
"#,
    // 4 - люди: фото профиля, отзывы и комментарии, друзья, переписка
    r#"
ALTER TABLE profiles ADD COLUMN photo TEXT;

-- Отзыв или комментарий к фильму; оценка - по желанию, в половинках звезды (1..10 = 0,5..5).
CREATE TABLE reviews (
  id INTEGER PRIMARY KEY,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  stars INTEGER CHECK (stars BETWEEN 1 AND 10),
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  edited_at INTEGER
);
CREATE INDEX reviews_title ON reviews(title_id, created_at DESC);
CREATE INDEX reviews_author ON reviews(profile_id, created_at DESC);

CREATE TABLE review_likes (
  review_id INTEGER NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  PRIMARY KEY (review_id, profile_id)
);

-- Дружба: заявка от a к b; принята - status = 'accepted' (одна строка на пару).
CREATE TABLE friends (
  a INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  b INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('pending', 'accepted')),
  created_at INTEGER NOT NULL,
  PRIMARY KEY (a, b),
  CHECK (a != b)
);
CREATE INDEX friends_b ON friends(b);

CREATE TABLE messages (
  id INTEGER PRIMARY KEY,
  from_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  to_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  -- Обсуждаем фильм: карточка в сообщении.
  title_id INTEGER REFERENCES titles(id) ON DELETE SET NULL,
  created_at INTEGER NOT NULL,
  read_at INTEGER
);
CREATE INDEX messages_to ON messages(to_id, read_at);
CREATE INDEX messages_pair ON messages(min(from_id, to_id), max(from_id, to_id), id);
"#,
    // 5 - один аккаунт без профилей; лёгкие кадры для широких карточек
    r#"
DELETE FROM profiles WHERE id NOT IN (SELECT MIN(id) FROM profiles GROUP BY user_id);
UPDATE profiles SET kids = 0, name = (SELECT substr(u.name, 1, 60) FROM users u WHERE u.id = profiles.user_id);
UPDATE sessions SET profile_id = NULL;
ALTER TABLE titles ADD COLUMN backdrop_sm TEXT;

-- Совместные списки: «смотрим вместе» - с друзьями, у каждого участника одинаковый.
CREATE TABLE lists (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  owner_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL
);
CREATE TABLE list_members (
  list_id INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  PRIMARY KEY (list_id, profile_id)
);
CREATE INDEX list_members_profile ON list_members(profile_id);
CREATE TABLE list_items (
  list_id INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  added_by INTEGER REFERENCES profiles(id) ON DELETE SET NULL,
  added_at INTEGER NOT NULL,
  -- Посмотрели вместе (отмечает любой участник).
  watched_at INTEGER,
  PRIMARY KEY (list_id, title_id)
);
"#,
    // 6 - Nova: лента IMDb (фильмы не с сервиса), отметки и просьбы хосту, группы в списках,
    // ссылки на видео с других сайтов, оформление профиля
    r#"
UPDATE settings SET value = 'Nova' WHERE key = 'site.name' AND value = 'Films & Serials For You';

-- Фильм или сериал с IMDb, которого может не быть на компьютере хоста.
CREATE TABLE ext_titles (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('movie', 'series')),
  title TEXT NOT NULL,
  original TEXT,
  year INTEGER,
  rating REAL,
  votes INTEGER,
  genres TEXT NOT NULL DEFAULT '[]',
  overview TEXT,
  runtime INTEGER,
  -- Картинка IMDb (адрес) и её копия у нас (`images/…`): браузер ходит только к нам.
  poster_url TEXT,
  poster TEXT,
  detailed INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL
);

-- Подборки IMDb: popular_movies, popular_series, top_movies, top_series.
CREATE TABLE ext_charts (
  chart TEXT NOT NULL,
  pos INTEGER NOT NULL,
  ext_id TEXT NOT NULL REFERENCES ext_titles(id) ON DELETE CASCADE,
  PRIMARY KEY (chart, pos)
);

-- «Буду смотреть» и «Просмотрел» для фильмов не с сервиса.
CREATE TABLE ext_marks (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  ext_id TEXT NOT NULL REFERENCES ext_titles(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('want', 'watched')),
  at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, ext_id)
);

-- «Хочу посмотреть на сервисе» - просьба хосту.
CREATE TABLE wishes (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  ext_id TEXT NOT NULL REFERENCES ext_titles(id) ON DELETE CASCADE,
  at INTEGER NOT NULL,
  dismissed_at INTEGER,
  PRIMARY KEY (profile_id, ext_id)
);

CREATE TABLE list_groups (
  id INTEGER PRIMARY KEY,
  list_id INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  pos INTEGER NOT NULL DEFAULT 0
);
ALTER TABLE list_items ADD COLUMN group_id INTEGER REFERENCES list_groups(id) ON DELETE SET NULL;
CREATE TABLE list_ext (
  list_id INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
  ext_id TEXT NOT NULL REFERENCES ext_titles(id) ON DELETE CASCADE,
  group_id INTEGER REFERENCES list_groups(id) ON DELETE SET NULL,
  added_by INTEGER REFERENCES profiles(id) ON DELETE SET NULL,
  added_at INTEGER NOT NULL,
  watched_at INTEGER,
  PRIMARY KEY (list_id, ext_id)
);

-- Видео с других сайтов: Rutube, VK Видео, YouTube - их плеером, прямые mp4/m3u8 - нашим.
-- У фильма с сервиса, у фильма из ленты или просто так.
CREATE TABLE links (
  id INTEGER PRIMARY KEY,
  title_id INTEGER REFERENCES titles(id) ON DELETE CASCADE,
  ext_id TEXT REFERENCES ext_titles(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('rutube', 'vk', 'youtube', 'direct')),
  url TEXT NOT NULL,
  embed TEXT,
  label TEXT NOT NULL,
  thumb TEXT,
  duration INTEGER,
  added_by INTEGER REFERENCES profiles(id) ON DELETE SET NULL,
  added_at INTEGER NOT NULL,
  CHECK (title_id IS NULL OR ext_id IS NULL)
);
CREATE INDEX links_title ON links(title_id);
CREATE INDEX links_ext ON links(ext_id);

-- Оформление профиля: обложка, цвет, «о себе», любимые фильмы (до 6: «t:12» или «x:tt…»).
ALTER TABLE profiles ADD COLUMN cover TEXT;
ALTER TABLE profiles ADD COLUMN accent TEXT;
ALTER TABLE profiles ADD COLUMN bio TEXT;
ALTER TABLE profiles ADD COLUMN favorites TEXT NOT NULL DEFAULT '[]';
"#,
    // 7 (0.7): большой кадр героя и страницы фильма - 1920 px вместо оригинала (бывает 4K и
    // несколько мегабайт: через туннель главная открывалась секундами).
    r#"
ALTER TABLE titles ADD COLUMN backdrop_md TEXT;
"#,
    // 8 (0.6.2): любимые режиссёры (до 5) и вид обложки профиля: «ribbon» - лента просмотренного
    // (по умолчанию), «image» - своя картинка.
    r#"
ALTER TABLE profiles ADD COLUMN directors TEXT NOT NULL DEFAULT '[]';
ALTER TABLE profiles ADD COLUMN cover_kind TEXT NOT NULL DEFAULT 'ribbon' CHECK (cover_kind IN ('ribbon','image'));
"#,
    // 9 (0.6.4): у фильма из ленты - своя страница: большой кадр (`backdrop`) и остальное
    // (слоган, актёры, кадры, похожие - JSON в `extra`). `detailed` = 2 - описание в этом виде.
    r#"
ALTER TABLE ext_titles ADD COLUMN backdrop TEXT;
ALTER TABLE ext_titles ADD COLUMN extra TEXT;
"#,
    // 10 (1.0): оценка серии; «Смотрю» вручную и «Больше не смотрю»; «Смотрю» и серии у сериалов
    // из ленты; отзывы к фильмам из ленты; показывать ли друзьям, что смотрю.
    r#"
ALTER TABLE files ADD COLUMN rating REAL;

-- «Смотрю» без начатых файлов (отметил сам).
CREATE TABLE watching (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, title_id)
);
-- «Больше не смотрю»: не в «Продолжить» и не «Смотрю», пока не начнёт снова (прогресс новее `at`).
CREATE TABLE dropped (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, title_id)
);

CREATE TABLE ext_marks_new (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  ext_id TEXT NOT NULL REFERENCES ext_titles(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('want', 'watching', 'watched')),
  at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, ext_id)
);
INSERT INTO ext_marks_new SELECT profile_id, ext_id, status, at FROM ext_marks;
DROP TABLE ext_marks;
ALTER TABLE ext_marks_new RENAME TO ext_marks;

-- Серии сериала из ленты (IMDb, по сезону - когда открыли), кадр - копия у нас.
CREATE TABLE ext_episodes (
  ext_id TEXT NOT NULL REFERENCES ext_titles(id) ON DELETE CASCADE,
  season INTEGER NOT NULL,
  episode INTEGER NOT NULL,
  name TEXT,
  overview TEXT,
  still_url TEXT,
  still TEXT,
  air_date TEXT,
  runtime INTEGER,
  rating REAL,
  PRIMARY KEY (ext_id, season, episode)
);
CREATE TABLE ext_seasons (
  ext_id TEXT NOT NULL REFERENCES ext_titles(id) ON DELETE CASCADE,
  season INTEGER NOT NULL,
  at INTEGER NOT NULL,
  PRIMARY KEY (ext_id, season)
);
-- Отмеченные серии сериала из ленты: «досюда посмотрел».
CREATE TABLE ext_seen (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  ext_id TEXT NOT NULL REFERENCES ext_titles(id) ON DELETE CASCADE,
  season INTEGER NOT NULL,
  episode INTEGER NOT NULL,
  at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, ext_id, season, episode)
);

-- Отзыв - к фильму с сервиса или из ленты. Таблицу пересобираем; лайки переносим (DROP TABLE
-- удалил бы их каскадом).
CREATE TABLE likes_keep AS SELECT review_id, profile_id FROM review_likes;
CREATE TABLE reviews_new (
  id INTEGER PRIMARY KEY,
  title_id INTEGER REFERENCES titles(id) ON DELETE CASCADE,
  ext_id TEXT REFERENCES ext_titles(id) ON DELETE CASCADE,
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  stars INTEGER CHECK (stars BETWEEN 1 AND 10),
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  edited_at INTEGER,
  CHECK ((title_id IS NULL) != (ext_id IS NULL))
);
INSERT INTO reviews_new (id, title_id, profile_id, stars, body, created_at, edited_at)
  SELECT id, title_id, profile_id, stars, body, created_at, edited_at FROM reviews;
DROP TABLE reviews;
ALTER TABLE reviews_new RENAME TO reviews;
CREATE INDEX reviews_title ON reviews(title_id, created_at DESC);
CREATE INDEX reviews_ext ON reviews(ext_id, created_at DESC);
CREATE INDEX reviews_author ON reviews(profile_id, created_at DESC);
INSERT OR IGNORE INTO review_likes (review_id, profile_id) SELECT review_id, profile_id FROM likes_keep;
DROP TABLE likes_keep;

ALTER TABLE profiles ADD COLUMN show_activity INTEGER NOT NULL DEFAULT 1;

-- Описания IMDb, полученные без второго запроса (IMDb переименовал поле - пропали актёры, кадры
-- и русские названия), - заново по тому же id.
UPDATE titles SET meta_status = 'pending' WHERE provider = 'imdb' AND meta_status = 'matched' AND locked = 0;
"#,
    // 11 (1.0): ссылки и со своих сайтов хоста (`site`: видео со страницы - в нашем плеере).
    r#"
CREATE TABLE links_new (
  id INTEGER PRIMARY KEY,
  title_id INTEGER REFERENCES titles(id) ON DELETE CASCADE,
  ext_id TEXT REFERENCES ext_titles(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('rutube', 'vk', 'youtube', 'direct', 'site')),
  url TEXT NOT NULL,
  embed TEXT,
  label TEXT NOT NULL,
  thumb TEXT,
  duration INTEGER,
  added_by INTEGER REFERENCES profiles(id) ON DELETE SET NULL,
  added_at INTEGER NOT NULL,
  CHECK (title_id IS NULL OR ext_id IS NULL)
);
INSERT INTO links_new SELECT id, title_id, ext_id, kind, url, embed, label, thumb, duration, added_by, added_at FROM links;
DROP TABLE links;
ALTER TABLE links_new RENAME TO links;
CREATE INDEX links_title ON links(title_id);
CREATE INDEX links_ext ON links(ext_id);
"#,
    // 12 (1.0): режиссёры и актёры - кто это на IMDb, фото и фильмография (id фильмов из ленты по годам).
    r#"
CREATE TABLE ext_people (
  id TEXT NOT NULL,
  -- director или actor: фильмография своя у каждой роли.
  role TEXT NOT NULL DEFAULT 'director',
  name TEXT NOT NULL,
  photo TEXT,
  works TEXT NOT NULL DEFAULT '[]',
  at INTEGER NOT NULL,
  PRIMARY KEY (id, role)
);
CREATE TABLE ext_people_names (
  name TEXT PRIMARY KEY,
  id TEXT NOT NULL,
  at INTEGER NOT NULL
);
"#,
    // 13 (1.0): где остановился у видео по ссылке (Rutube, VK, свой сайт) - «Продолжить просмотр».
    r#"
CREATE TABLE link_progress (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  link_id INTEGER NOT NULL REFERENCES links(id) ON DELETE CASCADE,
  position REAL NOT NULL,
  duration REAL NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, link_id)
);
"#,
    // 14 (1.0): хронология франшиз (Викиданные: «часть серии» и номер в ней) по id IMDb фильма.
    r#"
CREATE TABLE franchises (
  imdb TEXT PRIMARY KEY,
  -- [{name, ids: [tt…]}] - серии, где есть этот фильм, по порядку; [] - ни в одной.
  data TEXT NOT NULL,
  at INTEGER NOT NULL
);
"#,
    // 15: точная привязка внешнего видео к сезону и серии.
    r#"
ALTER TABLE links ADD COLUMN season INTEGER;
ALTER TABLE links ADD COLUMN episode INTEGER;
CREATE INDEX links_episode ON links(ext_id, season, episode);
CREATE TABLE ext_dropped (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  ext_id TEXT NOT NULL REFERENCES ext_titles(id) ON DELETE CASCADE,
  at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, ext_id)
);
UPDATE titles SET backdrop_md = NULL WHERE backdrop IS NOT NULL;
INSERT INTO settings(key, value) VALUES ('video.sites', '["vikinmult.net","garri-potter-lordfilms.com"]')
ON CONFLICT(key) DO UPDATE SET value = (
    SELECT json_group_array(domain) FROM (
        SELECT value AS domain FROM json_each(CASE WHEN json_valid(settings.value) THEN settings.value ELSE '[]' END)
        UNION SELECT 'vikinmult.net' UNION SELECT 'garri-potter-lordfilms.com'
    )
);
"#,
    // 16: проверенные сведения о людях из Викиданных и Википедии.
    r#"
CREATE TABLE person_info (id TEXT PRIMARY KEY, data TEXT NOT NULL, at INTEGER NOT NULL);
"#,
    // 17: локализованные названия и результаты поиска не зависят от подсказок IMDb.
    r#"
CREATE TABLE title_localizations (id TEXT PRIMARY KEY, title TEXT NOT NULL, data TEXT NOT NULL DEFAULT '{}', at INTEGER NOT NULL);
CREATE TABLE catalog_search_cache (query TEXT PRIMARY KEY, ids TEXT NOT NULL, at INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS progress_recent ON progress(profile_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS link_progress_recent ON link_progress(profile_id, updated_at DESC);
"#,
    // 18: подтверждённая почта и одноразовые токены восстановления.
    r#"
CREATE TABLE account_emails(user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,email TEXT NOT NULL UNIQUE COLLATE NOCASE);
CREATE TABLE account_tokens(hash TEXT PRIMARY KEY,user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,purpose TEXT NOT NULL CHECK(purpose IN ('verify','reset')),email TEXT NOT NULL,expires_at INTEGER NOT NULL,created_at INTEGER NOT NULL);
CREATE INDEX account_tokens_user ON account_tokens(user_id,purpose);
"#,
    // 19: «не тот фильм» - адреса видео, которые зрители отметили неверными: поиск их больше не предлагает.
    r#"
CREATE TABLE bad_videos (
  url TEXT PRIMARY KEY,
  title_id INTEGER,
  ext_id TEXT,
  by_profile INTEGER,
  at INTEGER NOT NULL
);
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
        conn.set_prepared_statement_cache_capacity(64);
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;",
        )?;
        let version: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > MIGRATIONS.len() {
            anyhow::bail!("база от более новой версии программы - обновите приложение");
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
        self.with(|c| {
            c.prepare_cached("SELECT value FROM settings WHERE key = ?")?.query_row([key], |r| r.get(0)).optional()
        })
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
        // Миграция 3 на уже открытой базе не повторяется - проверяем её запрос напрямую.
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

    #[test]
    fn reviews_rebuild_keeps_likes() {
        // База версии 9 с отзывом и лайком → миграция 10 пересобирает отзывы, лайк остаётся.
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        for sql in &MIGRATIONS[..9] {
            let tx = c.transaction().unwrap();
            tx.execute_batch(sql).unwrap();
            tx.commit().unwrap();
        }
        c.execute_batch(
            "INSERT INTO users (id, login, name, role, created_at) VALUES (1, 'a', 'A', 'host', 0);
             INSERT INTO profiles (id, user_id, name, created_at) VALUES (1, 1, 'A', 0);
             INSERT INTO titles (id, kind, key, title, added_at, updated_at) VALUES (1, 'movie', 'm', 'M', 0, 0);
             INSERT INTO reviews (id, title_id, profile_id, stars, body, created_at) VALUES (7, 1, 1, 8, 'Хорошо', 5);
             INSERT INTO review_likes (review_id, profile_id) VALUES (7, 1);
             INSERT INTO ext_titles (id, kind, title, updated_at) VALUES ('tt0000001', 'series', 'S', 0);
             INSERT INTO ext_marks (profile_id, ext_id, status, at) VALUES (1, 'tt0000001', 'want', 1);",
        )
        .unwrap();
        let tx = c.transaction().unwrap();
        tx.execute_batch(MIGRATIONS[9]).unwrap();
        tx.commit().unwrap();
        let likes: i64 =
            c.query_row("SELECT COUNT(*) FROM review_likes WHERE review_id = 7", [], |r| r.get(0)).unwrap();
        assert_eq!(likes, 1);
        // Отзыв к фильму из ленты и «Смотрю» у него - теперь можно.
        c.execute(
            "INSERT INTO reviews (ext_id, profile_id, body, created_at) VALUES ('tt0000001', 1, 'Сериал', 9)",
            [],
        )
        .unwrap();
        c.execute("UPDATE ext_marks SET status = 'watching'", []).unwrap();
        // Удалили отзыв - его лайки уходят каскадом, как раньше.
        c.execute("DELETE FROM reviews WHERE id = 7", []).unwrap();
        let likes: i64 = c.query_row("SELECT COUNT(*) FROM review_likes", [], |r| r.get(0)).unwrap();
        assert_eq!(likes, 0);
        assert!(c
            .execute("INSERT INTO reviews (profile_id, body, created_at) VALUES (1, 'ни к чему', 1)", [])
            .is_err());
    }

    #[test]
    fn one_profile_per_account() {
        let db = Db::memory().unwrap();
        let sql = MIGRATIONS[4].split("ALTER TABLE titles").next().unwrap();
        let (n, name, kids, other) = db
            .with(|c| {
                c.execute_batch(
                    "INSERT INTO users (id, login, name, role, created_at) VALUES (1, 'a', 'Анна', 'viewer', 0);
                     INSERT INTO profiles (id, user_id, name, kids, created_at) VALUES (1, 1, 'Мама', 1, 0), (2, 1, 'Дети', 1, 0);
                     INSERT INTO titles (id, kind, key, title, added_at, updated_at) VALUES (1, 'movie', 'm', 'M', 0, 0);
                     INSERT INTO my_list (profile_id, title_id, added_at) VALUES (2, 1, 0);",
                )?;
                c.execute_batch(sql)?;
                c.query_row(
                    "SELECT COUNT(*), MIN(name), MAX(kids), (SELECT COUNT(*) FROM my_list) FROM profiles",
                    [],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, bool>(2)?, r.get::<_, i64>(3)?)),
                )
            })
            .unwrap();
        assert_eq!((n, name.as_str(), kids, other), (1, "Анна", false, 0));
    }
}
