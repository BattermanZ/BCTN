# BCTN

## Build the names database (SQLite)

Prereqs: Rust toolchain.

Builds `data/bctn.sqlite` from the source files in `database_source/`:

```sh
cargo run --release -- --out data/bctn.sqlite --start-year 2005 --end-year 2024
```

