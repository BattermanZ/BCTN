# BCTN Project Progress Report

This document tracks the implementation progress against the `database_plan.md`.

## Summary

The initial database schema and the core normalization logic are now complete. This covers **Milestones 1 and 2 (M1, M2)** of the project plan. The project is now ready for the development of data ingestion scripts.

## Completed Items

- **[x] Schema & Extensions (Milestone 1):**
    - PostgreSQL extensions `unaccent` and `pg_trgm` are enabled.
    - The core schema (Section 1) and staging/audit schema (Section 2) have been created.
    - All tables, constraints, and indexes are in place.
    - The project uses a `.env` file for secure database credential management.
    - A pure SQL setup file is available at `bctn_database/001_initial_schema.sql`.

- **[x] Normalisation & Merge Policy (Milestone 2):**
    - A `bctn_core` module has been established for core application logic.
    - A `normalization.py` module was added with pure functions for:
        - Unicode normalization (NFC).
        - ASCII folding.
        - Slug generation.
    - Deduplication/equivalence logic has been implemented according to the business rules.
    - A full unit test suite (`tests/test_normalization.py`) has been created and verified.

## Next Steps (Pending Milestones)

- **[ ] M3 – Source mappings & first import (Section 4):** Develop scripts to fetch data from sources like Wikidata and Wikipedia and load it into the staging tables.
- **[ ] M4 – QA checks (Section 5):** Create SQL queries to verify the integrity and quality of the data.
- **[ ] M5 – Backups & docs (Sections 7 & 8):** Implement a backup strategy.