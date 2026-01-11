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
