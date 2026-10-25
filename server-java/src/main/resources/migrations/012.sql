
CREATE TABLE ext_people (
  id TEXT NOT NULL,
  -- director или actor: фильмография своя у каждой роли.
  role TEXT NOT NULL DEFAULT 'director',
  name TEXT NOT NULL,
  photo TEXT,
  works TEXT NOT NULL DEFAULT '[]',
  at INTEGER NOT NULL,
  PRIMARY KEY (id, role)
);
CREATE TABLE ext_people_names (
  name TEXT PRIMARY KEY,
  id TEXT NOT NULL,
  at INTEGER NOT NULL
);
