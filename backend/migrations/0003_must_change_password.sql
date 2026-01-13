PRAGMA foreign_keys = ON;

ALTER TABLE app_user
ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0;

