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

CREATE TABLE IF NOT EXISTS wikidata_concept_match (
  concept_id INTEGER NOT NULL REFERENCES name_concept(id) ON DELETE CASCADE,
  qid TEXT NOT NULL,
  matched_label TEXT NOT NULL,
  matched_label_lang TEXT,
  PRIMARY KEY (concept_id, qid, matched_label)
);

CREATE TABLE IF NOT EXISTS name_variant_edge (
  id INTEGER PRIMARY KEY,
  from_concept_id INTEGER NOT NULL REFERENCES name_concept(id) ON DELETE CASCADE,
  to_concept_id INTEGER NOT NULL REFERENCES name_concept(id) ON DELETE CASCADE,
  method TEXT NOT NULL,
  score REAL NOT NULL,
  status TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE (from_concept_id, to_concept_id, method)
);

-- Derived data (safe to rebuild): variant groups are connected components over accepted edges.
CREATE TABLE IF NOT EXISTS variant_group (
  variant_group_key TEXT PRIMARY KEY,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS variant_group_member (
  variant_group_key TEXT NOT NULL REFERENCES variant_group(variant_group_key) ON DELETE CASCADE,
  concept_key TEXT NOT NULL REFERENCES name_concept(concept_key) ON DELETE CASCADE,
  PRIMARY KEY (variant_group_key, concept_key)
);

-- Derived data (safe to rebuild): precomputed totals for filtering/scoring.
CREATE TABLE IF NOT EXISTS concept_country_window_total (
  concept_id INTEGER NOT NULL REFERENCES name_concept(id) ON DELETE CASCADE,
  country TEXT NOT NULL,
  total INTEGER NOT NULL,
  male INTEGER NOT NULL,
  female INTEGER NOT NULL,
  PRIMARY KEY (concept_id, country)
);

CREATE TABLE IF NOT EXISTS concept_country_top100_any (
  concept_id INTEGER NOT NULL REFERENCES name_concept(id) ON DELETE CASCADE,
  country TEXT NOT NULL,
  top100_any INTEGER NOT NULL,
  PRIMARY KEY (concept_id, country)
);

CREATE TABLE IF NOT EXISTS variant_group_country_window_total (
  variant_group_key TEXT NOT NULL REFERENCES variant_group(variant_group_key) ON DELETE CASCADE,
  country TEXT NOT NULL,
  total INTEGER NOT NULL,
  male INTEGER NOT NULL,
  female INTEGER NOT NULL,
  PRIMARY KEY (variant_group_key, country)
);

CREATE TABLE IF NOT EXISTS variant_group_country_top100_any (
  variant_group_key TEXT NOT NULL REFERENCES variant_group(variant_group_key) ON DELETE CASCADE,
  country TEXT NOT NULL,
  top100_any INTEGER NOT NULL,
  PRIMARY KEY (variant_group_key, country)
);
