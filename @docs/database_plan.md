# BCTN **Database** — Build Plan (No API) for Gemini CLI

**Scope:** Build the **database only** (schema, indexes, provenance/licensing, staging, deduplication rules, and ingestion specs). No web API, no UI. Output is a populated PostgreSQL database you can use later from any app.

---

## 0) Ground rules for Gemini

* **Language & tooling:** Use **PostgreSQL 16**. Generate **SQL DDL + Alembic migrations** (or plain SQL if preferred). Write small **CLI ingestion scripts** (Python 3.13) only if needed to load data — but keep the focus on DB.
* **Extensions:** Enable `unaccent` (accent-insensitive matching) and `pg_trgm` (fuzzy lookups) now to avoid painful migrations later.
* **Licensing hygiene:** Every field sourced externally must carry **provenance** and **licence** metadata (Wikidata = CC0; Wikipedia/Wiktionary = CC BY‑SA 4.0; commercial APIs per ToS). Store the source and licence **per-field or per-record**.
* **i18n & Unicode:** Store names in **NFC**; also keep an ASCII‑folded column for search.
* **Deliverable per step:** SQL migration(s) + short README note + acceptance checks.

---

## 1) Create the core schema

**Tables (must match exactly):**

* **names**: `id SERIAL PK`, `display TEXT NOT NULL` (NFC), `ascii TEXT NOT NULL`, `slug TEXT NOT NULL UNIQUE`, `primary_language TEXT CHECK (primary_language IN ('en','fr','es','unknown')) DEFAULT 'unknown'`, `gender_class TEXT CHECK (gender_class IN ('masculine','feminine','unisex','unknown')) DEFAULT 'unknown'`, `wikidata_qid TEXT NULL`, `notes TEXT NULL`, `created_at TIMESTAMPTZ DEFAULT now()`, `updated_at TIMESTAMPTZ DEFAULT now()`.
* **name\_usages**: `id SERIAL PK`, `name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE`, `language CHAR(2) NOT NULL`, `region CHAR(2) NULL`, `script TEXT NULL`, `kind TEXT CHECK (kind IN ('masculine','feminine','unisex','unknown')) NOT NULL`, `source TEXT NOT NULL CHECK (source IN ('wikidata','wikipedia','wiktionary','behindthename','ninjas','manual'))`, `notes TEXT NULL`, `created_at TIMESTAMPTZ DEFAULT now()`. **Unique**: (`name_id`,`language`,`kind`,`source`).
* **name\_relations**: `id SERIAL PK`, `a_name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE`, `b_name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE`, `relation TEXT CHECK (relation IN ('other_gender','variant','diminutive','cognate')) NOT NULL`, `source TEXT NOT NULL`, `created_at TIMESTAMPTZ DEFAULT now()`. **Unique**: (`a_name_id`,`b_name_id`,`relation`).
* **name\_aliases**: `id SERIAL PK`, `name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE`, `alias TEXT NOT NULL`, `language CHAR(2) NULL`, `source TEXT NOT NULL`, `created_at TIMESTAMPTZ DEFAULT now()`. Index on (`alias`, `language`).
* **name\_meanings**: `id SERIAL PK`, `name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE`, `short_meaning TEXT NOT NULL`, `long_meaning TEXT NULL`, `language CHAR(2) NOT NULL`, `source TEXT NOT NULL`, `licence TEXT NOT NULL`, `attribution_html TEXT NULL`, `url TEXT NULL`, `created_at TIMESTAMPTZ DEFAULT now()`. (Store text only when licence permits.)
* **name\_probs** (optional phase): `id SERIAL PK`, `name_id INT NOT NULL REFERENCES names(id) ON DELETE CASCADE`, `provider TEXT NOT NULL CHECK (provider IN ('genderize','nationalize','namsor','nameapi'))`, `metric TEXT NOT NULL CHECK (metric IN ('gender','country'))`, `value_json JSONB NOT NULL`, `sample_n INT NULL`, `last_checked_at TIMESTAMPTZ DEFAULT now()`.
* **source\_log**: `id SERIAL PK`, `provider TEXT NOT NULL`, `request_sig TEXT NOT NULL`, `status INT NOT NULL`, `fetched_at TIMESTAMPTZ DEFAULT now()`, `rate_window_left INT NULL`, `notes TEXT NULL`.

**Indexes:**

* `CREATE INDEX ON names USING gin (slug gin_trgm_ops);`
* `CREATE INDEX ON names (ascii);` and `CREATE INDEX ON names (display);`
* `CREATE INDEX ON name_aliases (alias);`
* Foreign‑key indexes for all `*_id` fields.

**Acceptance Criteria**

* Migrations/DDL run clean. `unaccent` and `pg_trgm` enabled. Constraints present; unique rules enforced.

---

## 2) Add staging & audit for safe imports

Create **staging** tables to land raw records before merging, plus an audit trail. This keeps core tables clean and dedupe idempotent.

* **stage\_items**: `id SERIAL PK`, `provider TEXT NOT NULL`, `external_id TEXT NULL`, `payload JSONB NOT NULL`, `fetched_at TIMESTAMPTZ DEFAULT now()`, `hash TEXT NOT NULL`, **Unique** (`provider`,`hash`).
* **stage\_edges** (relations between staged items): `id SERIAL PK`, `provider TEXT NOT NULL`, `from_external_id TEXT`, `to_external_id TEXT`, `relation TEXT NOT NULL`, `payload JSONB`, `hash TEXT NOT NULL`, **Unique** (`provider`,`hash`).
* **import\_jobs**: `id SERIAL PK`, `provider TEXT NOT NULL`, `started_at TIMESTAMPTZ`, `finished_at TIMESTAMPTZ`, `status TEXT CHECK (status IN ('pending','running','success','error'))`, `notes TEXT`.
* **import\_errors**: `id SERIAL PK`, `job_id INT REFERENCES import_jobs(id) ON DELETE CASCADE`, `context TEXT`, `payload JSONB`, `error TEXT`, `created_at TIMESTAMPTZ DEFAULT now()`.

**Acceptance Criteria**

* You can insert dummy staged rows and run a merge script that upserts into `names` without creating duplicates when re‑run.

---

## 3) Normalisation & merge policy (write as README rules + tests)

* **Unicode policy:** Convert all name inputs to NFC. Compute `ascii = unaccent(lower(display))` and `slug = replace(ascii, ' ', '-')` (strip punctuation sensibly).
* **Deduping heuristics (in order):**

  1. Exact `display` match **and** same `language` within `name_usages` → same name.
  2. Case/diacritic‑insensitive `display` match **and** same `language` **and** compatible `gender_class` → merge.
  3. If sources state they are variants (e.g., BTN related; Wikidata `P1560` other‑gender), keep separate `names` but add `name_relations`.
* **Gender precedence:** curated (`wikidata`, `behindthename`) > probabilistic providers. Conflicts → set `gender_class='unknown'` on `names` and keep specifics in `name_usages.notes`.
* **Provenance:** Never overwrite curated fields with probabilistic outputs. Always record `source`.

**Acceptance Criteria**

* Re‑running the same merge on the same staging set yields **no new rows** in `names` (idempotency proven by tests/queries).

---

## 4) Source‑specific mapping specs (database‑only view)

### 4.0 Exact IDs & category titles (copy–paste for ingestion)

**Wikidata items (QIDs):**

* Given name — `Q202444`
* Male given name — `Q12308941`
* Female given name — `Q11879590`
* Unisex given name — `Q3409032`

**Wikidata properties:**

* `P407` — language of work or name
* `P1560` — given name version for other gender

**Language QIDs to filter `P407`:**

* English `Q1860` · French `Q150` · Spanish `Q1321`

**Wikipedia categories to enumerate (page titles):**

* **English**: `Category:English given names`; `Category:English-language masculine given names`; `Category:English-language feminine given names`; `Category:English-language unisex given names`
* **French**: `Category:French given names`; `Category:French masculine given names`; `Category:French feminine given names`; `Category:French unisex given names`
* **Spanish**: `Category:Spanish given names`; `Category:Spanish masculine given names`; `Category:Spanish feminine given names`; `Category:Spanish unisex given names`

**Wiktionary (optional backfill):**

* French: `Category:French given names` (+ male/female)
* Spanish: `Category:Spanish given names` (+ male/female/unisex)

> These are **specifications** for what the database expects from ingestion; keep as comments/docstrings in your merge scripts. No API work.

### 4.1 Wikidata (WDQS/SPARQL)

* **Target items:** Instances (or subclasses) of **given name (Q202444)** with `P31` in {male given name, female given name, unisex given name}. Use `P407` (language of work or name) when available to tag EN/FR/ES. Use `skos:altLabel` for aliases. Use `P1560` for other‑gender links.
* **DB mapping:**

  * label → `names.display`; aliases → `name_aliases.alias`.
  * gender class (male/female/unisex) → `names.gender_class`.
  * languages (EN/FR/ES) → `name_usages(language, kind)`.
  * QID → `names.wikidata_qid`.
  * other‑gender links → `name_relations(relation='other_gender')`.
* **Licence:** CC0. Record `source='wikidata'`.

### 4.2 Wikipedia / Wiktionary (MediaWiki categories)

* **Target categories:** “English given names”, “French given names”, “Spanish given names” **and** their gendered subcats (feminine/masculine/unisex).
* **DB mapping:**

  * Page title (cleaned of “(given name)” suffixes) → `names.display`.
  * Category gender → `name_usages.kind`.
  * Optional extracts/etymologies (short) → `name_meanings` (store `licence='CC-BY-SA-4.0'` and attribution/url).
* **Licence:** CC BY‑SA 4.0 for text; titles only are generally OK but still store `source='wikipedia'`.

### 4.3 Behind the Name (BTN)

* **Use:** Provide curated **meaning** text, **usage** tags (eng/fre/spa), and **related** links (variants/diminutives/cognates).
* **DB mapping:**

  * Meanings → `name_meanings` (with `licence` + `attribution_html` + `url`).
  * Usage → additional rows in `name_usages`.
  * Related → `name_relations` with appropriate `relation`.
* **Constraints:** Respect BTN request caps; store provenance.

### 4.4 Optional fillers / enrichment (later)

* **API Ninjas**: add extra names only (mark as `source='ninjas'`).
* **Genderize/Nationalize/NamSor/NameAPI**: store only in `name_probs` with raw JSON.

**Acceptance Criteria**

* After initial WD import + WP backfill, DB contains thousands of EN/FR/ES names with correct `gender_class`, `name_usages`, and some relations.

### 4.5 Ready‑to‑run WDQS SPARQL queries (paste into [https://query.wikidata.org/](https://query.wikidata.org/))

> **Tip:** Run per **first‑letter partitions** (`FILTER(STRSTARTS(LCASE(?nameLabel), "a"))`) to keep results small. Use the **strict** query first (with `P407` language), then the **broad** fallback if coverage is thin.

**A) Strict: EN/FR/ES given names with language tag (P407) and gender class**

```sparql
# Given names with explicit language-of-name in EN/FR/ES and gender class
SELECT ?name ?nameLabel ?qid ?genderClassLabel ?langLabel WHERE {
  VALUES ?genderClass { wd:Q12308941 wd:Q11879590 wd:Q3409032 } # male/female/unisex
  VALUES ?lang { wd:Q1860 wd:Q150 wd:Q1321 }                    # EN/FR/ES
  ?name wdt:P31 ?genderClass .
  ?name wdt:P407 ?lang .
  BIND(STRAFTER(STR(?name), "entity/") AS ?qid)
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
  # Optional letter partition for large pulls:
  # FILTER(STRSTARTS(LCASE(?nameLabel), "a"))
}
ORDER BY ?nameLabel
LIMIT 10000
```

**B) Broad fallback: EN/FR/ES candidates by gender class only (no P407)**

```sparql
# Broader: all given names by gender class (language will be resolved via Wikipedia categories later)
SELECT ?name ?nameLabel ?qid ?genderClassLabel WHERE {
  VALUES ?genderClass { wd:Q12308941 wd:Q11879590 wd:Q3409032 }
  ?name wdt:P31 ?genderClass .
  BIND(STRAFTER(STR(?name), "entity/") AS ?qid)
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
  # Optional partition:
  # FILTER(STRSTARTS(LCASE(?nameLabel), "a"))
}
ORDER BY ?nameLabel
LIMIT 10000
```

**C) Other‑gender pairs (P1560) to populate `name_relations`**

```sparql
# Map other-gender versions (e.g., Paul <-> Paula)
SELECT ?a ?aLabel ?aQ ?b ?bLabel ?bQ WHERE {
  ?a wdt:P1560 ?b .
  BIND(STRAFTER(STR(?a), "entity/") AS ?aQ)
  BIND(STRAFTER(STR(?b), "entity/") AS ?bQ)
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
ORDER BY ?aLabel
LIMIT 10000
```

**D) Aliases for names (to fill `name_aliases`)**

```sparql
# Fetch aliases for given-name items (English labels shown; aliases returned in all languages)
SELECT ?name ?nameLabel ?qid ?alias WHERE {
  VALUES ?genderClass { wd:Q12308941 wd:Q11879590 wd:Q3409032 }
  ?name wdt:P31 ?genderClass .
  ?name skos:altLabel ?alias .
  BIND(STRAFTER(STR(?name), "entity/") AS ?qid)
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
ORDER BY ?nameLabel
LIMIT 20000
```

**E) Sanity counts**

```sparql
# Quick counts by gender class
SELECT ?genderClassLabel (COUNT(?name) AS ?n) WHERE {
  VALUES ?genderClass { wd:Q12308941 wd:Q11879590 wd:Q3409032 }
  ?name wdt:P31 ?genderClass .
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
GROUP BY ?genderClassLabel
ORDER BY DESC(?n)
```

### 4.6 MediaWiki category fetches (ready URLs & parameters)

Use `action=query&list=categorymembers` and paginate with `cmcontinue`. Fetch **page titles** first; pull short extracts later only if you plan to store text with attribution.

**Base endpoint:**

```
https://en.wikipedia.org/w/api.php?action=query&list=categorymembers&format=json&cmlimit=500&cmtitle=Category:English_given_names
```

**Examples (swap domain/language and `cmtitle`):**

* English feminine: `Category:English-language_feminine_given_names`
* English masculine: `Category:English-language_masculine_given_names`
* English unisex:   `Category:English-language_unisex_given_names`
* French feminine:  `Category:French_feminine_given_names`
* French masculine: `Category:French_masculine_given_names`
* French unisex:   `Category:French-language_unisex_given_names`
* Spanish feminine: `Category:Spanish_feminine_given_names`
* Spanish masculine:`Category:Spanish_masculine_given_names`
* Spanish unisex:   `Category:Spanish_unisex_given_names`

**Politeness settings:** add `&maxlag=5` and set a custom `User-Agent`. Serialize requests or keep concurrency very low.

**Mapping note:** strip trailing `" (given name)"` from titles when present and normalise per Section 3.

---

## 5) Quality gates & checks (write SQL checks and keep in /docs)

* **Uniqueness:**

  * `slug` unique; no duplicates on (`name_id`,`language`,`kind`,`source`).
* **Integrity:**

  * All `name_relations` reference existing `names` and have `a_name_id != b_name_id`.
* **Sanity:**

  * No `display` values that are all‑ASCII but differ from `ascii` (flag normalisation bugs).
  * Sample queries prove that `Aurélien` ↔ search by `Aurelien` and `José` ↔ `Jose` match when using `unaccent`.
* **Licensing:**

  * Any row with `name_meanings` has `licence` set and a non‑null `source`.

**Acceptance Criteria**

* All checks pass; report saved to `/docs/DB_QA.md` with row counts and any exceptions.

---

## 6) Performance & indexing plan (DB‑only)

* Keep `pg_trgm` on `names.slug` for fuzzy matching; add `GIN` index on `name_aliases.alias`.
* For later heavy lookups, consider materialised view `mv_names_search` combining names + aliases + usages (not required now).

**Acceptance Criteria**

* Example fuzzy queries on mid‑sized data sets complete in milliseconds (demonstrate with `EXPLAIN ANALYZE`).

---

## 7) Backups & migration discipline

* Nightly `pg_dump` of schema + data to a Synology share; retain **7 daily + 4 weekly**.
* Every schema change via migration; never edit tables manually.

**Acceptance Criteria**

* A full restore works into a fresh Postgres and passes quality gates.

---

## 8) Prompts for Gemini CLI (database‑only)

> Use these prompts verbatim, one step at a time.

**A. Schema & extensions**
“Create SQL (and Alembic migrations) to define the tables and constraints from sections 1 and 2, and enable `unaccent` and `pg_trgm`. Include indexes exactly as specified.”

**B. Normalisation helpers**
“Generate a small Python module with pure functions to normalise names (NFC, ASCII fold via `unidecode` or Postgres `unaccent`), build slugs, and compare equality under the rules in section 3. Include unit tests.”

**C. Staging→Core merge**
“Create a SQL/PLpgSQL merge script that reads `stage_items` and `stage_edges`, applies the dedupe heuristics from section 3, and upserts into `names`, `name_usages`, `name_aliases`, and `name_relations`. Idempotent; safe to re‑run.”

**D. Source mappers (spec‑only)**
“Write mapping docs (YAML or Markdown) that show how each source (Wikidata, Wikipedia/Wiktionary, BTN) should populate the staging tables, aligning fields to the DB mapping in section 4. No API code — just the mapping doc and example staged rows.”

**E. Quality gates**
“Produce SQL checks described in section 5 and a script that outputs a QA report to `/docs/DB_QA.md`.”

**F. Backups**
“Create a shell script to run nightly `pg_dump` with rotation (7 daily, 4 weekly) and store to a configurable path.”

---

## 9) Milestones (DB‑only)

1. **M1 – Schema & extensions** (Sections 1 & 2).
2. **M2 – Normalisation & merge policy** (Section 3).
3. **M3 – Source mappings & first import** (Section 4).
4. **M4 – QA checks** (Section 5).
5. **M5 – Backups & docs** (Sections 7 & 8).
