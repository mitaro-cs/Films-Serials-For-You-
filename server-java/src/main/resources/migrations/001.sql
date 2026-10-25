
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
