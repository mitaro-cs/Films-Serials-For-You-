
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
