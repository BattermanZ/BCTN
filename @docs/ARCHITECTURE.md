# BCTN — Architecture (PWA + Rust Backend)

This document proposes a concrete architecture for the Tinder-like baby-name matching app, based on decisions recorded in `@docs/PRODUCT_DECISIONS.md`.

## High-level Components

- **Names DB (existing):** `data/bctn.sqlite`
  - Built via `bctn-ingest ingest` + variant link generation via `bctn-ingest variants build` and `bctn-ingest variants build-ensemble`.
  - Contains name concepts, yearly observations, and `name_variant_edge` links.
  - Rebuildable (idempotent ingest currently recreates the file), so it should not store user accounts/swipes.

- **App DB (new):** `data/bctn_app.sqlite`
  - Source of truth for: accounts, swipes, matches, per-user settings, and (eventually) the curated shortlist/final picks.
  - References names by stable keys (see “Identifiers”), not by internal SQLite row IDs that may change on re-ingest.

- **Backend (Rust):** single server process
  - Serves the PWA (static assets) and JSON APIs.
  - Auth (username+password), admin UI (protected), real-time updates for matches/likes.

- **Frontend (PWA, iOS Safari):**
  - Swipe deck UI (Like/Nope/Super-like).
  - Matches screen.
  - “Likes” and “Super-likes” screens.
  - Detail sheet for a card (popularity per country, ranks last 5 years, spellings).
  - Toggle: append last name “Zarca Wong” on the swipe card.

## Identifiers (critical for rebuild safety)

The ingestion pipeline recreates `data/bctn.sqlite`, which means integer IDs may change across rebuilds.

To make user data stable, the app should reference names using **stable keys**:

- **Concept key:** `name_concept.concept_key` (folded key).
  - This is deterministic given the normalization rules and raw name input.

- **Variant group key (computed):** `variant_group_key`
  - Defined as the **minimum concept_key** within the accepted-edge connected component.
  - Used for matching and for “overarching variant group” display.

The swipe deck operates on **cards**, where a card is either:
- a variant-group card (preferred when a group exists), or
- a single-concept card (fallback if no accepted group exists).

## Variant Groups (how groups are formed)

Groups are connected components over **accepted** edges in `name_variant_edge`:

- Use edges where:
  - `status = 'accepted'`
  - and `method IN ('wikidata','ensemble')` (configurable; default to both)

Group formation logic:
1. Build an undirected graph between concepts.
2. Compute connected components (Union-Find/DSU).
3. For each component, compute `variant_group_key = min(concept_key)` (lexicographic).

### Materialization strategy

For performance and simplicity in the server, materialize group membership into a table/view inside the **Names DB**:

- `variant_group`
  - `variant_group_key TEXT PRIMARY KEY`
  - `created_at INTEGER`

- `variant_group_member`
  - `variant_group_key TEXT NOT NULL`
  - `concept_key TEXT NOT NULL`
  - `PRIMARY KEY(variant_group_key, concept_key)`

This can be rebuilt any time from `name_variant_edge` and is safe because it remains derived data from the Names DB.

If we do not want to change the Names DB schema, the backend can compute DSU at startup and keep an in-memory map, but materialization is preferred for:
- easier SQL queries,
- less memory in server,
- simpler join logic for spellings and popularity.

## Popularity (per million births) and “Top” tier

Popularity is **per evidence country** (US/FR/CL) and computed as:

- `births_per_million = (name_births / total_births) * 1_000_000`

Where:
- `name_births` is the sum of `name_observation.count` for all concepts in the card (variant group) within the fixed 20-year window.
- `total_births` is the sum of `name_observation.count` for all names in that country within the fixed 20-year window.

Tiers:
- Extra rare: `< 75 / million` (hidden by default; opt-in)
- Rare: `75–300 / million`
- Common: `>= 300 / million` but not Top
- Top: **top 100** names in one or more countries (US/FR/CL) for the selected time window.
  - Window: fixed 20 years (whatever is present in `data/bctn.sqlite`).

### Detail sheet: rank in last 5 years

When the user opens the detail sheet for a card:
- For each selected country and for the most recent 5 years available in the Names DB:
  - compute rank within that country-year by total births.

Implementation options:
- **On-demand SQL** using window functions (simple, potentially heavy).
- **Precompute** a `country_year_rank` table for top K ranks per year (recommended if it becomes slow).

## Gender categorization (per country)

Gender is computed per evidence country using sex distribution:

- Unisex: neither sex > 70%
- Mostly male/female: 70–90%
- Male/female: > 90%

Multi-country display:
- If any selected country labels the card as Unisex, show “Unisex” and indicate which country/countries cause that label.

## Spellings (card + detail sheet)

Card display:
- Show 3 spellings:
  - Prefer top 3 spellings per selected country; if identical across selected countries, show top 3 global spellings.
  - Include “within-concept variants” using `name_observation.name_raw` for all concepts in the card.

Detail sheet:
- Show all spellings:
  - within each concept (observed raw spellings),
  - and across the whole variant group.

## Backend (Rust) Design

### Frameworks (proposed)
- HTTP server: `axum`
- Async runtime: `tokio`
- App DB access: `sqlx` (SQLite) with a connection pool
- Password hashing: `argon2` (argon2id)
- Auth sessions: server-side sessions + HttpOnly cookie
- Real-time: SSE (`/api/events`) or WebSocket (SSE is simpler)

### Databases

1) Names DB: `data/bctn.sqlite` (read-only for the server)
2) App DB: `data/bctn_app.sqlite` (read-write)

### App DB schema (v1)

- `app_user`
  - `id INTEGER PRIMARY KEY`
  - `username TEXT UNIQUE NOT NULL`
  - `password_hash TEXT NOT NULL`
  - `is_admin INTEGER NOT NULL` (0/1)
  - `created_at INTEGER NOT NULL`
  - `updated_at INTEGER NOT NULL`
  - `deleted_at INTEGER` (nullable; soft delete)
  - Enforce max 2 active users via trigger or application logic.

- `app_session`
  - `id INTEGER PRIMARY KEY`
  - `user_id INTEGER NOT NULL`
  - `session_token_hash TEXT NOT NULL` (store hash of a random token)
  - `created_at INTEGER NOT NULL`
  - `expires_at INTEGER NOT NULL`
  - `last_seen_at INTEGER NOT NULL`

- `user_settings`
  - `user_id INTEGER PRIMARY KEY`
  - `show_last_name INTEGER NOT NULL` (0/1)
  - `last_name TEXT NOT NULL` (default “Zarca Wong”)
  - `best_pick_mode INTEGER NOT NULL` (0/1, default 0)

- `swipe`
  - `id INTEGER PRIMARY KEY`
  - `user_id INTEGER NOT NULL`
  - `card_key TEXT NOT NULL` (variant_group_key OR concept_key fallback)
  - `card_kind TEXT NOT NULL` ('group'|'concept')
  - `action TEXT NOT NULL` ('like'|'nope'|'superlike')
  - `created_at INTEGER NOT NULL`
  - `UNIQUE(user_id, card_key)`

- `match`
  - `card_key TEXT PRIMARY KEY`
  - `created_at INTEGER NOT NULL`
  - Can be derived, but storing enables real-time notifications and a stable match list.

### API surface (v1)

Auth:
- `POST /api/auth/login` (username/password) -> sets session cookie
- `POST /api/auth/logout`
- `GET /api/me`

Settings:
- `GET /api/settings`
- `PATCH /api/settings` body: `{ show_last_name?, last_name?, best_pick_mode? }`

Admin:
- `GET /api/admin/users`
- `POST /api/admin/users` (create)
- `PATCH /api/admin/users/:id` (modify username/password/is_admin)
- `DELETE /api/admin/users/:id` (delete)
  - Must enforce “max 2 accounts total”.

Feed:
- `GET /api/feed/next?countries=US,FR&pop=rare|common|top&include_extra_rare=0|1&gender=...`
  - returns one card (or N cards) with:
    - `card_key`, `card_kind`
    - top 3 spellings
    - unisex label + which countries cause it
    - per-country per-million values (for badges)
  - ordering:
    - default: random within filters
    - best pick mode: ranked using `@docs/SCORING.md` (always global over FR/US/CL; fixed 20-year window), then tie-break randomly

Swipes:
- `POST /api/swipes` body: `{ card_key, card_kind, action }`
  - writes `swipe`
  - if action is like/superlike: checks partner swipe to create `match`
  - emits match event over SSE
- `POST /api/swipes/undo` (undo last action; server tracks last swipe per user)

Lists:
- `GET /api/likes` (both users’ likes, since likes are shared)
- `GET /api/superlikes` (both users’ superlikes, since shared)
- `GET /api/matches`

Detail:
- `GET /api/cards/:card_key`
  - returns:
    - all spellings (within concept + group)
    - per-country per-million over window
    - last-5-years per-country ranks (if available)
    - optional: trend sparkline data

Real-time:
- `GET /api/events` (SSE stream)
  - emits: `match_created`, `like_created`, `superlike_created` (Nope remains private)

## Open Implementation Notes / Risks

- **Wikidata edges may link non-spelling variants** (e.g., diminutives). If we only want spelling variants in groups, we may need:
  - a stricter acceptance policy for Wikidata edges, or
  - separate “grouping edges” vs “related-name edges”.
- **Performance:** “rank last 5 years” can be heavy if computed on-demand; may require precompute tables.
- **No reset:** deck eventually exhausts; need a clear “no more names” UI state.

## Next Steps

1) Confirm the two-DB approach (`data/bctn.sqlite` for names, `data/bctn_app.sqlite` for app state).
2) Decide whether variant groups are materialized in Names DB tables or computed in memory at server start.
3) Implement Rust server binary/subcommand (`bctn serve`) and the App DB schema migrations.
