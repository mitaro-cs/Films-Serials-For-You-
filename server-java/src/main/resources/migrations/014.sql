
CREATE TABLE franchises (
  imdb TEXT PRIMARY KEY,
  -- [{name, ids: [tt…]}] - серии, где есть этот фильм, по порядку; [] - ни в одной.
  data TEXT NOT NULL,
  at INTEGER NOT NULL
);
