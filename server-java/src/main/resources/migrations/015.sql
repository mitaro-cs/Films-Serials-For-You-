
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
