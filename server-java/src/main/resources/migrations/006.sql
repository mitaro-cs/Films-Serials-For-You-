
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
