
ALTER TABLE profiles ADD COLUMN directors TEXT NOT NULL DEFAULT '[]';
ALTER TABLE profiles ADD COLUMN cover_kind TEXT NOT NULL DEFAULT 'ribbon' CHECK (cover_kind IN ('ribbon','image'));
