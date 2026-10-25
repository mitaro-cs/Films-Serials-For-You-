
ALTER TABLE profiles ADD COLUMN photo TEXT;

-- Отзыв или комментарий к фильму; оценка - по желанию, в половинках звезды (1..10 = 0,5..5).
CREATE TABLE reviews (
  id INTEGER PRIMARY KEY,
  title_id INTEGER NOT NULL REFERENCES titles(id) ON DELETE CASCADE,
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  stars INTEGER CHECK (stars BETWEEN 1 AND 10),
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  edited_at INTEGER
);
CREATE INDEX reviews_title ON reviews(title_id, created_at DESC);
CREATE INDEX reviews_author ON reviews(profile_id, created_at DESC);

CREATE TABLE review_likes (
  review_id INTEGER NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
  profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  PRIMARY KEY (review_id, profile_id)
);

-- Дружба: заявка от a к b; принята - status = 'accepted' (одна строка на пару).
CREATE TABLE friends (
  a INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  b INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('pending', 'accepted')),
  created_at INTEGER NOT NULL,
  PRIMARY KEY (a, b),
  CHECK (a != b)
);
CREATE INDEX friends_b ON friends(b);

CREATE TABLE messages (
  id INTEGER PRIMARY KEY,
  from_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  to_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  -- Обсуждаем фильм: карточка в сообщении.
  title_id INTEGER REFERENCES titles(id) ON DELETE SET NULL,
  created_at INTEGER NOT NULL,
  read_at INTEGER
);
CREATE INDEX messages_to ON messages(to_id, read_at);
CREATE INDEX messages_pair ON messages(min(from_id, to_id), max(from_id, to_id), id);
