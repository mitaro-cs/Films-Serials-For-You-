
CREATE TABLE title_localizations (id TEXT PRIMARY KEY, title TEXT NOT NULL, data TEXT NOT NULL DEFAULT '{}', at INTEGER NOT NULL);
CREATE TABLE catalog_search_cache (query TEXT PRIMARY KEY, ids TEXT NOT NULL, at INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS progress_recent ON progress(profile_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS link_progress_recent ON link_progress(profile_id, updated_at DESC);
