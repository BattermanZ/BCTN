# BCTN — Product Decisions Log

This document tracks open questions and decisions for the baby-name “Tinder-like” matching app.

## Platform

**Decision:** iOS web app (PWA).

**Notes:**
- Mobile-first UI.
- Installed via Safari “Add to Home Screen”.
- Backend still required to sync swipes and compute matches across partners.

## Identity & Pairing (Open)

**Decision:** Exactly 2 users (admin-managed), no invitation/pairing flow.

**Admin requirements:**
- Admin can create, modify, and delete user accounts.

**Auth method:**
- Username + password.

## Matching Semantics (Open)

**Question:** What counts as “the same name” for matching?
- Option A: Exact `concept_id` match only.
- Option B: Match by variant-group (e.g., `Caitlin` ↔ `Caitlyn`) using accepted `name_variant_edge`.

**Decision:** Match by variant-group when available; otherwise match by concept.

**Display rule:**
- If a concept belongs to a variant-group, show the group (one canonical display name) instead of individual concepts.
- If a concept has no variant-group, show the concept directly.

**Spelling display:**
- Default: show 3 spellings:
  - Prefer top 3 spellings per selected country filter; if the top spellings are identical across selected countries, show top 3 global spellings.
  - Include spellings that are variants within the same concept (from `name_observation.name_raw` via concept membership).
- Optional: user can expand to “see all spellings” for the group/concept.

## Filters (Open)

### Locale / Origin
**Question:** Should “country” filter mean:
- evidence country (US/FR/CL observed in data), or
- etymological origin (requires separate data), or
- both (two different filters)?

**Decision:** Evidence country (US/FR/CL observed in data).

### Popularity
**Question:** How should popularity be defined for filtering/ranking?
- total occurrences across the chosen time window
- percentile / rank within a country/year
- “top N per year”

**Decision:** Use births per million births (per selected country evidence), plus a “top” tier.

**Tiers (per million births):**
- Extra rare: `< 75 / million` (hidden by default; opt-in)
- Rare: `75–300 / million`
- Common: between Rare and Top (i.e., >= 300 / million but not Top)
- Top: Top 100 **in one or more years** of the selected time window, in one or more evidence countries (US/FR/CL).

### Gender
**Question:** How should gender filtering work?
- strict (M/F/U)
- based on distribution threshold (e.g., ≥80% male → M)
- allow “unisex” category

**Decision:** Use sex distribution thresholds (computed from observation totals within the selected evidence countries + time window).

**Categories:**
- Unisex: neither sex > 70% (evaluated per country)
- Mostly male / mostly female: 70–90% (per country)
- Male / female: > 90% (per country)

**Display rule (multi-country):**
- If any selected country classifies the name as Unisex, show it as Unisex and indicate which country/countries drive that label.

## Feed & UX (Open)

**Question:** How should names be ordered in the swipe feed?
- random within filters
- popularity-weighted
- diversity-boosted (avoid too many similar names in a row)

**Decision:** Random within the selected filters (per user).

**Deck stability:**
- Fresh shuffle each time the app is opened.

**Resurfacing:**
- Names you already swiped should not be shown again (unless history is reset).

**Reset:**
- No “reset swipes” button.

## Modes (Decided)

**Default mode:**
- Default feed ordering is random within selected filters.

**Best pick mode (optional):**
- Off by default.
- When enabled, the feed uses the scoring system defined in `@docs/SCORING.md` to rank names (“best picks”).
- Scoring inputs are always global over the 3 evidence countries (US/FR/CL), even if the user filters to a subset of countries.
- Scoring window is fixed to the last 20 years (whatever is present in the Names DB).

## Swipe Card UI (Decided)

**Last name preview:**
- Show the name as “`<FirstName> Zarca Wong`” on the swipe card.
- Provide a toggle to hide/show the last name.

**Gender display:**
- Show how unisex it is (based on per-country sex distribution thresholds above).

**Details sheet:**
- A details sheet is accessible from the swipe card and shows:
  - popularity per country, including rank in that country for the last 5 years (if available)
  - all spellings:
    - all spellings within each concept (from observations)
    - and all spellings across the variant group (linked concepts)

## Swipe Actions (Decided)

- Like
- Nope
- Super-like

**Super-like behavior:**
- Super-like adds the name to a personal shortlist even if the partner has not liked it.

## Lists / History (Decided)

- Each user can view:
  - all personal likes
  - all personal super-likes (shortlist)

## Undo (Decided)

- Users can undo the last swipe action.

## Privacy (Open)

**Question:** Should a user’s likes be hidden until the other user has also swiped that name?

**Decision:** Likes and super-likes are visible to both users; dislikes (Nope) are private.

## Admin UI (Decided)

- A simple protected admin screen inside the same PWA.

**Admin auth:**
- Admin is a normal account with an `is_admin` flag.

**Account limit:**
- Hard-limit to exactly 2 accounts total.

## Offline (Decided)

- Online required (no offline swipe queue).

## Matches (Decided)

- Matches are created/visible in real-time when both users Like the same variant group (or same concept if no group exists).
- A dedicated Matches screen also shows the full list of matches.
