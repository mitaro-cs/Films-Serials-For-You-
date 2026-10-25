
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
