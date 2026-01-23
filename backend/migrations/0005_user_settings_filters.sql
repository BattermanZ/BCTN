PRAGMA foreign_keys = ON;

-- Persist "last used" filters per user so the app restores them after login.
-- Stored as comma-separated lists for simplicity (only two users; small data).

ALTER TABLE user_settings ADD COLUMN filter_countries TEXT NOT NULL DEFAULT 'US,FR,CL';
ALTER TABLE user_settings ADD COLUMN filter_popularity TEXT NOT NULL DEFAULT 'rare,common,top';
ALTER TABLE user_settings ADD COLUMN filter_gender TEXT NOT NULL DEFAULT 'any';

