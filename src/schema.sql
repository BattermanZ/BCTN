PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS source (
  id INTEGER PRIMARY KEY,
  source_key TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  country TEXT NOT NULL,
  url TEXT,
  license TEXT
);

CREATE TABLE IF NOT EXISTS import_run (
  id INTEGER PRIMARY KEY,
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  window_start_year INTEGER NOT NULL,
  window_end_year INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS name_string (
  id INTEGER PRIMARY KEY,
  display TEXT NOT NULL,
  norm TEXT NOT NULL UNIQUE,
  key_strict TEXT NOT NULL,
  key_folded TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_name_string_key_strict ON name_string(key_strict);
CREATE INDEX IF NOT EXISTS idx_name_string_key_folded ON name_string(key_folded);

CREATE TABLE IF NOT EXISTS name_concept (
  id INTEGER PRIMARY KEY,
  concept_key TEXT NOT NULL UNIQUE,
  preferred_name_string_id INTEGER NOT NULL REFERENCES name_string(id)
);

CREATE TABLE IF NOT EXISTS name_concept_member (
  concept_id INTEGER NOT NULL REFERENCES name_concept(id) ON DELETE CASCADE,
  name_string_id INTEGER NOT NULL REFERENCES name_string(id) ON DELETE CASCADE,
  relation_type TEXT NOT NULL,
  PRIMARY KEY (concept_id, name_string_id)
);

CREATE INDEX IF NOT EXISTS idx_name_concept_member_name_string ON name_concept_member(name_string_id);

CREATE TABLE IF NOT EXISTS name_observation (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES import_run(id) ON DELETE CASCADE,
  source_id INTEGER NOT NULL REFERENCES source(id),
  name_string_id INTEGER NOT NULL REFERENCES name_string(id),
  name_raw TEXT NOT NULL,
  country TEXT NOT NULL,
  year INTEGER NOT NULL,
  sex TEXT NOT NULL,
  sex_raw TEXT NOT NULL,
  count INTEGER NOT NULL,
  proportion REAL
);

CREATE INDEX IF NOT EXISTS idx_name_observation_country_year ON name_observation(country, year);
CREATE INDEX IF NOT EXISTS idx_name_observation_name_string ON name_observation(name_string_id);
CREATE INDEX IF NOT EXISTS idx_name_observation_year ON name_observation(year);
CREATE INDEX IF NOT EXISTS idx_name_observation_name_raw ON name_observation(name_raw);
