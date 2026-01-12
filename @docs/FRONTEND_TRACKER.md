# BCTN — Frontend Tracker (SvelteKit PWA)

This document tracks frontend decisions, open questions, and implementation checkpoints for the Tinder-like baby-name app.

## Stack (Decided)

- Framework: **SvelteKit**
- Language: **TypeScript**
- Package manager: **pnpm**
- Target: **iOS PWA** (Safari “Add to Home Screen”)
- Deployment model: **single deploy** (Rust backend serves the built PWA assets from the same origin).

## Decisions Log

- 2026-01-12: Choose SvelteKit for “native iOS feel” UI.
- 2026-01-12: Rendering mode: SPA (client-side).
- 2026-01-12: Styling: Tailwind.
- 2026-01-12: Build/deploy: `adapter-static` with `fallback: index.html`; output served from `web/build/`.

## Rendering & Routing (Decided)

- Rendering: **SPA** (`ssr = false`)
- Routing: client-side via SvelteKit, with static hosting fallback (`index.html`) for deep links.

## Styling (Decided)

- Tailwind (global CSS in `web/src/app.css`)

## Build Output & Backend Integration (Decided)

- SvelteKit adapter: `@sveltejs/adapter-static` with `fallback: index.html`.
- Build output directory: `web/build/`.
- Backend serves:
  - PWA under `/` from `web/build/` (configurable via `BCTN_STATIC_DIR`)
  - JSON API under `/api/*` from the same origin.

## Core Screens (Planned)

- Login
- Swipe deck
- Detail sheet (name stats + spellings)
- Matches
- Likes (shared)
- Super-likes (shared) + personal view (TBD)
- Filters + settings
- Admin (accounts management)

## Swipe Deck UX (Requirements)

- Tinder-like card gestures (Nope/Like/Super-like)
- Undo last swipe
- Name preview: `<FirstName> Zarca Wong` with a toggle to hide/show last name
- Show “how unisex” it is (per-country label, with per-country notes)
- Feed order:
  - default random within filters
  - optional best-pick mode: sorted by `@docs/SCORING.md` (off by default)

## Data Display Rules (Requirements)

- Cards show variant-group if it exists; else show concept.
- Show 3 spellings:
  - prefer top spellings per selected countries; if identical across selected countries, show global top spellings
  - include within-concept variants
  - allow “see all spellings”

## Backend API Contract (Current)

Auth:
- `POST /api/auth/login`
- `POST /api/auth/logout`
- `GET /api/me`

Settings:
- `GET /api/settings`
- `PATCH /api/settings`

Feed + swipes:
- `GET /api/feed/next`
- `POST /api/swipes`
- `POST /api/swipes/undo`

Lists:
- `GET /api/matches`
- `GET /api/likes`
- `GET /api/superlikes`

Admin:
- `GET /api/admin/users`
- `POST /api/admin/users`
- `PATCH /api/admin/users/:id`
- `DELETE /api/admin/users/:id`

## Open Questions

1) Do we need iOS-specific UX extras:
   - haptic feedback (limited in web)
   - motion reduction toggle
   - offline behavior (currently: online required)
2) “Personal likes” vs “shared likes” views:
   - Product decisions say likes/superlikes are visible to both users; should UI still show “mine” and “partner” sections?
