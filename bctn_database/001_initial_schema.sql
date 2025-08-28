-- BCTN Initial Schema
-- This file is generated from the first Alembic migration.

-- Enable extensions
CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Create core schema tables
CREATE TABLE names (
    id SERIAL PRIMARY KEY,
    display TEXT NOT NULL,
    ascii TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    primary_language TEXT CHECK (primary_language IN ('en','fr','es','unknown')) DEFAULT 'unknown',
    gender_class TEXT CHECK (gender_class IN ('masculine','feminine','unisex','unknown')) DEFAULT 'unknown',
    wikidata_qid TEXT NULL,
    notes TEXT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE name_usages (
    id SERIAL PRIMARY KEY,
    name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE,
    language CHAR(2) NOT NULL,
    region CHAR(2) NULL,
    script TEXT NULL,
    kind TEXT CHECK (kind IN ('masculine','feminine','unisex','unknown')) NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('wikidata','wikipedia','wiktionary','behindthename','ninjas','manual')),
    notes TEXT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (name_id, language, kind, source)
);

CREATE TABLE name_relations (
    id SERIAL PRIMARY KEY,
    a_name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE,
    b_name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE,
    relation TEXT CHECK (relation IN ('other_gender','variant','diminutive','cognate')) NOT NULL,
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (a_name_id, b_name_id, relation)
);

CREATE TABLE name_aliases (
    id SERIAL PRIMARY KEY,
    name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    language CHAR(2) NULL,
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE name_meanings (
    id SERIAL PRIMARY KEY,
    name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE,
    short_meaning TEXT NOT NULL,
    long_meaning TEXT NULL,
    language CHAR(2) NOT NULL,
    source TEXT NOT NULL,
    licence TEXT NOT NULL,
    attribution_html TEXT NULL,
    url TEXT NULL,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE name_probs (
    id SERIAL PRIMARY KEY,
    name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('genderize','nationalize','namsor','nameapi')),
    metric TEXT NOT NULL CHECK (metric IN ('gender','country')),
    value_json JSONB NOT NULL,
    sample_n INT NULL,
    last_checked_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE source_log (
    id SERIAL PRIMARY KEY,
    provider TEXT NOT NULL,
    request_sig TEXT NOT NULL,
    status INT NOT NULL,
    fetched_at TIMESTAMPTZ DEFAULT now(),
    rate_window_left INT NULL,
    notes TEXT NULL
);

-- Create staging and audit tables
CREATE TABLE stage_items (
    id SERIAL PRIMARY KEY,
    provider TEXT NOT NULL,
    external_id TEXT NULL,
    payload JSONB NOT NULL,
    fetched_at TIMESTAMPTZ DEFAULT now(),
    hash TEXT NOT NULL,
    UNIQUE (provider, hash)
);

CREATE TABLE stage_edges (
    id SERIAL PRIMARY KEY,
    provider TEXT NOT NULL,
    from_external_id TEXT,
    to_external_id TEXT,
    relation TEXT NOT NULL,
    payload JSONB,
    hash TEXT NOT NULL,
    UNIQUE (provider, hash)
);

CREATE TABLE import_jobs (
    id SERIAL PRIMARY KEY,
    provider TEXT NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    status TEXT CHECK (status IN ('pending','running','success','error')),
    notes TEXT
);

CREATE TABLE import_errors (
    id SERIAL PRIMARY KEY,
    job_id INT REFERENCES import_jobs(id) ON DELETE CASCADE,
    context TEXT,
    payload JSONB,
    error TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- Create indexes
CREATE INDEX idx_names_slug_gin ON names USING gin (slug gin_trgm_ops);
CREATE INDEX idx_names_ascii ON names (ascii);
CREATE INDEX idx_names_display ON names (display);
CREATE INDEX idx_name_aliases_alias ON name_aliases (alias);
CREATE INDEX idx_name_aliases_alias_language ON name_aliases (alias, language);

-- Create foreign-key indexes
CREATE INDEX idx_name_usages_name_id ON name_usages (name_id);
CREATE INDEX idx_name_relations_a_name_id ON name_relations (a_name_id);
CREATE INDEX idx_name_relations_b_name_id ON name_relations (b_name_id);
CREATE INDEX idx_name_aliases_name_id ON name_aliases (name_id);
CREATE INDEX idx_name_meanings_name_id ON name_meanings (name_id);
CREATE INDEX idx_name_probs_name_id ON name_probs (name_id);
CREATE INDEX idx_import_errors_job_id ON import_errors (job_id);

-- Alembic version table - required for future migrations
CREATE TABLE alembic_version (
    version_num VARCHAR(32) NOT NULL,
    CONSTRAINT alembic_version_pkc PRIMARY KEY (version_num)
);

-- Stamp the database with the revision of the created schema
INSERT INTO alembic_version (version_num) VALUES ('b71138a2554e');

