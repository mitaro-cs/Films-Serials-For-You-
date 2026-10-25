
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
