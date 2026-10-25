
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
