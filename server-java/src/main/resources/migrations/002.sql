
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
