# BCTN Project Progress Report

This document tracks the implementation progress against the `database_plan.md`.

## Summary

The initial database schema setup is complete. This corresponds to **Milestone 1 (M1)** of the project plan. The database is now ready for data normalization rules and ingestion scripts to be developed.

## Completed Items (Milestone 1)

- **[x] PostgreSQL Extensions:**
    - `unaccent` has been enabled for accent-insensitive searching.
    - `pg_trgm` has been enabled for fuzzy/trigram-based searching.

- **[x] Core Schema (Section 1):**
    - All core tables have been created:
        - `names`
        - `name_usages`
        - `name_relations`
        - `name_aliases`
        - `name_meanings`
        - `name_probs`
        - `source_log`
    - All specified constraints (Primary Keys, Foreign Keys, Unique, Check) are in place.
    - All specified indexes, including GIN/trigram indexes and foreign key indexes, have been created.

- **[x] Staging & Audit Schema (Section 2):**
    - All staging and audit tables have been created:
        - `stage_items`
        - `stage_edges`
        - `import_jobs`
        - `import_errors`

- **[x] Migration and Setup:**
    - The database schema is defined in an Alembic migration and a pure SQL script (`bctn_database/001_initial_schema.sql`).
    - The project is configured to use a `.env` file for securely managing database credentials, with separate variables for the user, password, host, port, and database name.

## Next Steps (Pending Milestones)

- **[ ] M2 – Normalisation & merge policy (Section 3):** Develop Python functions and tests for normalizing and deduplicating name data.
- **[ ] M3 – Source mappings & first import (Section 4):** Develop scripts to fetch data from sources like Wikidata and Wikipedia and load it into the staging tables.
- **[ ] M4 – QA checks (Section 5):** Create SQL queries to verify the integrity and quality of the data.
- **[ ] M5 – Backups & docs (Sections 7 & 8):** Implement a backup strategy.
