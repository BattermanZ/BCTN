# BCTN

## Build the names database (SQLite)

Prereqs: Rust toolchain.

Builds `data/bctn.sqlite` from the source files in `database_source/`:

```sh
cargo run --release -- ingest --out data/bctn.sqlite --start-year 2005 --end-year 2024
```

## Browse/search (CLI)

```sh
# search by folded key (case/accents/apostrophes folded; hyphens preserved)
cargo run --release -- search jose

# substring search (slower)
cargo run --release -- search --contains allah

# show a concept (term is folded to a concept key)
cargo run --release -- show abdallah

# top names by country/year
cargo run --release -- top --country US --year 2024 --limit 25
```

## Variant edges (Wikidata)

Build automatic variant edges (global edges with per-country evidence):

```sh
cargo run --release -- variants build --db data/bctn.sqlite --max-concepts 1000 --batch-size 50 --request-sleep-ms 500

# to refresh the on-disk cache:
cargo run --release -- variants build --db data/bctn.sqlite --refresh-cache
```

## Variant edges (Ensemble scoring)

Build variant edges using an ensemble of signals (string distance, within-country trend correlation, sex agreement, collision penalties) with optional Wikidata auto-accept when both concepts map to the same QID:

```sh
# run globally (ALL countries)
cargo run --release -- variants build-ensemble --db data/bctn.sqlite

# run for a single country and keep it fast
cargo run --release -- variants build-ensemble --db data/bctn.sqlite --country FR --max-concepts 2000 --min-total 20

# delete previously-generated ensemble edges and rebuild
cargo run --release -- variants build-ensemble --db data/bctn.sqlite --clear-ensemble
```
