mod cl;
mod fr;
mod us;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};

use crate::cli::IngestArgs;
use crate::db;
use crate::normalize;

#[derive(Debug, Clone, Copy)]
enum Country {
    Us,
    Fr,
    Cl,
}

impl Country {
    fn as_str(self) -> &'static str {
        match self {
            Country::Us => "US",
            Country::Fr => "FR",
            Country::Cl => "CL",
        }
    }
}

#[derive(Debug, Clone)]
struct NameDerived {
    display: String,
    norm: String,
    key_strict: String,
    key_folded: String,
}

#[derive(Debug)]
struct IngestContext {
    conn: Connection,
    run_id: i64,
    caches: IngestCaches,
}

#[derive(Debug)]
struct IngestCaches {
    name_string_by_norm: HashMap<String, i64>,
    concept_by_key: HashMap<String, i64>,
}

pub(crate) fn run(args: IngestArgs) -> Result<()> {
    if args.start_year > args.end_year {
        bail!("start_year must be <= end_year");
    }

    let Some(parent) = args.out.parent() else {
        bail!("output path must have a parent directory");
    };
    fs::create_dir_all(parent).with_context(|| format!("create dir {parent:?}"))?;
    if args.out.exists() {
        fs::remove_file(&args.out).with_context(|| format!("remove existing db {:?}", args.out))?;
    }

    let mut ingest = IngestContext::new(&args.out, args.start_year, args.end_year)?;
    let sources = insert_sources(&ingest.conn)?;

    us::ingest_us(
        &mut ingest,
        sources.us_ssa,
        &args.database_source_dir.join("US_names"),
        args.start_year,
        args.end_year,
    )?;
    fr::ingest_fr(
        &mut ingest,
        sources.fr_insee,
        &args.database_source_dir.join("French_names.csv"),
        args.start_year,
        args.end_year,
    )?;
    cl::ingest_cl(
        &mut ingest,
        sources.cl_guaguas,
        &args.database_source_dir.join("CL_names/1920-2021.csv"),
        args.start_year,
        args.end_year,
    )?;

    ingest.finish_run()?;
    Ok(())
}

impl IngestContext {
    fn new(out: &Path, start_year: i32, end_year: i32) -> Result<Self> {
        let conn = Connection::open(out).with_context(|| format!("open sqlite db {out:?}"))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
        conn.pragma_update(None, "cache_size", "-200000")?;

        db::ensure_schema(&conn)?;
        conn.execute_batch("BEGIN IMMEDIATE")?;

        let started_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        conn.execute(
            "INSERT INTO import_run (started_at, window_start_year, window_end_year) VALUES (?1, ?2, ?3)",
            params![started_at, start_year, end_year],
        )?;
        let run_id = conn.last_insert_rowid();

        Ok(Self {
            conn,
            run_id,
            caches: IngestCaches {
                name_string_by_norm: HashMap::new(),
                concept_by_key: HashMap::new(),
            },
        })
    }

    fn finish_run(&mut self) -> Result<()> {
        db::ensure_indexes(&self.conn)?;
        let finished_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        self.conn.execute(
            "UPDATE import_run SET finished_at = ?1 WHERE id = ?2",
            params![finished_at, self.run_id],
        )?;
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }
}

impl IngestCaches {
    fn get_or_create_name_string(
        &mut self,
        conn: &Connection,
        display: &str,
    ) -> Result<(i64, NameDerived)> {
        let derived = derive_name(display);

        if let Some(id) = self.name_string_by_norm.get(&derived.norm).copied() {
            return Ok((id, derived));
        }

        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM name_string WHERE norm = ?1",
                params![derived.norm],
                |row| row.get(0),
            )
            .optional()?;

        let id = if let Some(id) = existing {
            id
        } else {
            conn.execute(
                "INSERT INTO name_string (display, norm, key_strict, key_folded) VALUES (?1, ?2, ?3, ?4)",
                params![derived.display, derived.norm, derived.key_strict, derived.key_folded],
            )?;
            conn.last_insert_rowid()
        };

        self.name_string_by_norm.insert(derived.norm.clone(), id);
        Ok((id, derived))
    }

    fn get_or_create_concept(
        &mut self,
        conn: &Connection,
        concept_key: &str,
        preferred_name_string_id: i64,
    ) -> Result<i64> {
        if let Some(id) = self.concept_by_key.get(concept_key).copied() {
            return Ok(id);
        }

        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM name_concept WHERE concept_key = ?1",
                params![concept_key],
                |row| row.get(0),
            )
            .optional()?;

        let id = if let Some(id) = existing {
            id
        } else {
            conn.execute(
                "INSERT INTO name_concept (concept_key, preferred_name_string_id) VALUES (?1, ?2)",
                params![concept_key, preferred_name_string_id],
            )?;
            conn.last_insert_rowid()
        };

        self.concept_by_key.insert(concept_key.to_owned(), id);
        Ok(id)
    }
}

fn derive_name(display: &str) -> NameDerived {
    let norm = normalize::norm_display(display);
    let key_strict = normalize::key_strict(&norm);
    let key_folded = normalize::key_folded(&key_strict);
    NameDerived {
        display: display.to_owned(),
        norm,
        key_strict,
        key_folded,
    }
}

struct SourceIds {
    us_ssa: i64,
    fr_insee: i64,
    cl_guaguas: i64,
}

fn insert_sources(conn: &Connection) -> Result<SourceIds> {
    let mut insert = conn.prepare(
        "INSERT INTO source (source_key, name, country, url, license) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(source_key) DO UPDATE SET
           name = excluded.name,
           country = excluded.country,
           url = excluded.url,
           license = excluded.license",
    )?;

    insert.execute(params![
        "us_ssa",
        "US SSA baby names (yob*.txt)",
        Country::Us.as_str(),
        "https://www.ssa.gov/oact/babynames/limits.html",
        "Public domain (US government)",
    ])?;
    let us_ssa = conn.query_row(
        "SELECT id FROM source WHERE source_key = 'us_ssa'",
        [],
        |r| r.get(0),
    )?;

    insert.execute(params![
        "fr_insee",
        "France INSEE first names",
        Country::Fr.as_str(),
        "https://www.insee.fr/fr/statistiques/2540004",
        "See https://www.insee.fr/fr/information/2382777",
    ])?;
    let fr_insee = conn.query_row(
        "SELECT id FROM source WHERE source_key = 'fr_insee'",
        [],
        |r| r.get(0),
    )?;

    insert.execute(params![
        "cl_guaguas",
        "Chile Civil Registry baby names (guaguas)",
        Country::Cl.as_str(),
        "https://raw.githubusercontent.com/rivaquiroga/guaguas/main/data-raw/1920-2021.csv",
        "CC0-1.0",
    ])?;
    let cl_guaguas = conn.query_row(
        "SELECT id FROM source WHERE source_key = 'cl_guaguas'",
        [],
        |r| r.get(0),
    )?;

    Ok(SourceIds {
        us_ssa,
        fr_insee,
        cl_guaguas,
    })
}

fn ensure_membership(
    conn: &Connection,
    concept_id: i64,
    name_string_id: i64,
    relation_type: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO name_concept_member (concept_id, name_string_id, relation_type) VALUES (?1, ?2, ?3)",
        params![concept_id, name_string_id, relation_type],
    )?;
    Ok(())
}
