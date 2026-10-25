
CREATE TABLE link_progress (
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  link_id INTEGER NOT NULL REFERENCES links(id) ON DELETE CASCADE,
  position REAL NOT NULL,
  duration REAL NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (profile_id, link_id)
);
