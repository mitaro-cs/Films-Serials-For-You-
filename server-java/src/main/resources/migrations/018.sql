
CREATE TABLE account_emails(user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,email TEXT NOT NULL UNIQUE COLLATE NOCASE);
CREATE TABLE account_tokens(hash TEXT PRIMARY KEY,user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,purpose TEXT NOT NULL CHECK(purpose IN ('verify','reset')),email TEXT NOT NULL,expires_at INTEGER NOT NULL,created_at INTEGER NOT NULL);
CREATE INDEX account_tokens_user ON account_tokens(user_id,purpose);
