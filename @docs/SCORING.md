# Baby-name scoring system (FR / US / CL)

This document defines a simple, deterministic scoring system to **rank first names** using three evidence countries (**France, United States, Chile**) over a **selected time window** (e.g., 2010–2024).

It is designed to:

* Prefer names that are **used across all three countries** (cross-lingual practicality)
* Deprioritise names that are **Top 100** in any country (avoid overexposed names)

---

## Inputs required (per name × country)

For each name and each country `c ∈ {FR, US, CL}`, compute over the selected time window:

1. **Popularity rate** `ppm_c`

* `ppm_c` = *average births per year* **per million births** in that country.

2. **Top flag** `top100_c`

* `top100_c = true` if the name is **Top 100** in **one or more years** of the window **in that country**.

> Note: `ppm` makes popularity comparable across countries; `top100` captures “headline popularity”.

---

## Tiers (per country)

Using `ppm_c` and `top100_c`:

* **Extra rare (hidden by default)**: `ppm_c < 75`
* **Rare**: `75 ≤ ppm_c < 300`
* **Common**: `ppm_c ≥ 300` **and** `top100_c = false`
* **Top**: `top100_c = true`

---

## Step 1 — Per-country tier score

Convert each country’s tier into a numeric score:

* Extra rare → `tier_score_c = 0`
* Rare → `tier_score_c = 1`
* Common → `tier_score_c = 2`
* Top → `tier_score_c = 3`

---

## Step 2 — Base score

Sum the three tier scores:

`base = tier_score_FR + tier_score_US + tier_score_CL`

Range: `0 … 9`.

---

## Step 3 — Cross-country presence bonus

Reward names that are **not extra-rare** in multiple countries.

Let:

* `present_count = number of countries where ppm_c ≥ 75`

Bonus:

* If `present_count = 3` → `bonus_present = +6`
* If `present_count = 2` → `bonus_present = +3`
* If `present_count = 1` → `bonus_present = +0`

---

## Step 4 — Top penalty

Penalise “overexposed” names.

Let:

* `top_count = number of countries where top100_c = true`

Penalty:

* `penalty_top = 5 × top_count`

This makes the system naturally surface **“best picks”**: names that are Rare/Common across countries but **not Top** anywhere.

---

## Step 5 — “distance from Top” sweet-spot bonus

Goal: among **Common** names, prefer the ones that are **comfortably popular** but **not close to Top**.

For each country `c` where the name is **Common** (`ppm_c ≥ 300` and `top100_c = false`), apply:

* If `300 ≤ ppm_c < 800` → `sweet_c = +1`
* Else → `sweet_c = +0`

Then:

`sweet = sweet_FR + sweet_US + sweet_CL`

> This is intentionally tiny: it only nudges sorting within already-good candidates.

---

## Final score

`score = base + bonus_present + sweet − penalty_top`

Higher is better.

---

## Interpretation (what tends to rank high)

### High-scoring “best picks”

* Rare/Common in **all 3** countries (`present_count = 3`)
* **Not** Top 100 anywhere (`top_count = 0`)
* Ideally Common but in the **300–800 ppm** band in at least one country

### Names that drop

* Anything Top 100 in one or more countries (Top penalty)
* Names present in only one country (no presence bonus)

---

## Examples (illustrative)

### Example A — Common in all 3, not Top anywhere

* `tier_score`: 2 + 2 + 2 = 6
* `present_count = 3` → +6
* `top_count = 0` → −0
* `sweet` depends on ppm band

Score (without sweet): `6 + 6 = 12`

### Example B — Top in US, Common elsewhere

* `tier_score`: 3 + 2 + 2 = 7
* `present_count = 3` → +6
* `top_count = 1` → −5

Score: `7 + 6 − 5 = 8`

### Example C — Rare in all 3, not Top anywhere

* `tier_score`: 1 + 1 + 1 = 3
* `present_count = 3` → +6
* `top_count = 0` → −0

Score: `3 + 6 = 9`

---

## Notes / knobs you can tune later

* **Top penalty weight (5)**: increase to avoid Top names more aggressively; decrease to let them compete.
* **Sweet-spot band (300–800 ppm)**: widen/narrow to match your taste.
* **Presence bonus (+6/+3)**: increase if cross-country usability is a top priority.
