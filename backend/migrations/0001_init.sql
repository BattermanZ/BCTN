PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS app_user (
  id INTEGER PRIMARY KEY,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  is_admin INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  deleted_at INTEGER
);

CREATE TABLE IF NOT EXISTS app_session (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL,
  session_token_hash TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  last_seen_at INTEGER NOT NULL,
  FOREIGN KEY(user_id) REFERENCES app_user(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_settings (
  user_id INTEGER PRIMARY KEY,
  show_last_name INTEGER NOT NULL DEFAULT 1,
  last_name TEXT NOT NULL DEFAULT 'Zarca Wong',
  best_pick_mode INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY(user_id) REFERENCES app_user(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS swipe (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL,
  card_key TEXT NOT NULL,
  card_kind TEXT NOT NULL,
  action TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(user_id, card_key),
  FOREIGN KEY(user_id) REFERENCES app_user(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS name_match (
  card_key TEXT PRIMARY KEY,
  created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_swipe_user_created_at ON swipe(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_session_user ON app_session(user_id);

