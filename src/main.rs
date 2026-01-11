mod normalize;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::Parser;
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Parser)]
#[command(name = "bctn", version)]
struct Cli {
    #[arg(long, default_value = "database_source")]
    database_source_dir: PathBuf,

    #[arg(long, default_value = "data/bctn.sqlite")]
    out: PathBuf,

    #[arg(long, default_value_t = 2005)]
    start_year: i32,

    #[arg(long, default_value_t = 2024)]
    end_year: i32,
}

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

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.start_year > cli.end_year {
        bail!("start_year must be <= end_year");
    }

    let Some(parent) = cli.out.parent() else {
        bail!("output path must have a parent directory");
    };
    fs::create_dir_all(parent).with_context(|| format!("create dir {parent:?}"))?;
    if cli.out.exists() {
        fs::remove_file(&cli.out).with_context(|| format!("remove existing db {:?}", cli.out))?;
    }

    let mut ingest = IngestContext::new(&cli.out, cli.start_year, cli.end_year)?;

    let sources = insert_sources(&ingest.conn)?;
    ingest_us(
        &mut ingest,
        sources.us_ssa,
        &cli.database_source_dir.join("US_names"),
        cli.start_year,
        cli.end_year,
    )?;
    ingest_fr(
        &mut ingest,
        sources.fr_insee,
        &cli.database_source_dir.join("French_names.csv"),
        cli.start_year,
        cli.end_year,
    )?;
    ingest_cl(
        &mut ingest,
        sources.cl_guaguas,
        &cli.database_source_dir.join("CL_names/1920-2021.csv"),
        cli.start_year,
        cli.end_year,
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

        conn.execute_batch(include_str!("schema.sql"))?;
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
        "Unknown",
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
        "Unknown",
        "Unknown",
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

fn ingest_us(
    ingest: &mut IngestContext,
    source_id: i64,
    dir: &Path,
    start_year: i32,
    end_year: i32,
) -> Result<()> {
    if !dir.exists() {
        bail!("missing US source dir: {dir:?}");
    }

    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|n| n.starts_with("yob") && n.ends_with(".txt"))
        })
        .collect();
    entries.sort();

    let conn = &ingest.conn;
    let caches = &mut ingest.caches;

    let mut obs_stmt = conn.prepare(
        "INSERT INTO name_observation
         (run_id, source_id, name_string_id, country, year, sex, sex_raw, count, proportion)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
    )?;

    for path in entries {
        let year = parse_us_year(&path)?;
        if year < start_year || year > end_year {
            continue;
        }

        let contents = fs::read_to_string(&path).with_context(|| format!("read {path:?}"))?;
        for (line_no, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let (name, sex_raw, count) =
                parse_us_line(line).with_context(|| format!("parse {path:?}:{line_no}"))?;
            let (name_string_id, derived) = caches.get_or_create_name_string(conn, name)?;
            let concept_id =
                caches.get_or_create_concept(conn, &derived.key_folded, name_string_id)?;
            ensure_membership(conn, concept_id, name_string_id, "auto_folded")?;

            obs_stmt.execute(params![
                ingest.run_id,
                source_id,
                name_string_id,
                Country::Us.as_str(),
                year,
                sex_raw,
                sex_raw,
                count,
            ])?;
        }
    }

    Ok(())
}

fn parse_us_year(path: &Path) -> Result<i32> {
    let file = path
        .file_name()
        .and_then(OsStr::to_str)
        .context("missing filename")?;
    if !file.starts_with("yob") || !file.ends_with(".txt") {
        bail!("unexpected US file name: {file}");
    }
    let year_str = &file[3..file.len() - 4];
    let year: i32 = year_str
        .parse()
        .with_context(|| format!("parse year from {file}"))?;
    Ok(year)
}

fn parse_us_line(line: &str) -> Result<(&str, &str, i64)> {
    let mut parts = line.split(',');
    let name = parts.next().context("missing name")?;
    let sex = parts.next().context("missing sex")?;
    let count_str = parts.next().context("missing count")?;
    if parts.next().is_some() {
        bail!("too many columns");
    }
    if sex != "M" && sex != "F" {
        bail!("unexpected sex: {sex}");
    }
    let count: i64 = count_str.parse().context("parse count")?;
    Ok((name, sex, count))
}

fn ingest_fr(
    ingest: &mut IngestContext,
    source_id: i64,
    path: &Path,
    start_year: i32,
    end_year: i32,
) -> Result<()> {
    if !path.exists() {
        bail!("missing France source file: {path:?}");
    }

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_path(path)
        .with_context(|| format!("open {path:?}"))?;

    let conn = &ingest.conn;
    let caches = &mut ingest.caches;

    let mut obs_stmt = conn.prepare(
        "INSERT INTO name_observation
         (run_id, source_id, name_string_id, country, year, sex, sex_raw, count, proportion)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
    )?;

    for result in reader.records() {
        let record = result?;
        let sexe_raw = record.get(0).context("missing sexe")?;
        let name = record.get(1).context("missing preusuel")?;
        let ann = record.get(2).context("missing annais")?;
        let count_str = record.get(3).context("missing nombre")?;

        if ann == "XXXX" {
            continue;
        }
        let year: i32 = ann.parse().with_context(|| format!("parse year {ann}"))?;
        if year < start_year || year > end_year {
            continue;
        }
        if name.starts_with('_') {
            continue;
        }

        let sex = match sexe_raw {
            "1" => "M",
            "2" => "F",
            _ => "U",
        };
        let count: i64 = count_str.parse().context("parse count")?;

        let (name_string_id, derived) = caches.get_or_create_name_string(conn, name)?;
        let concept_id = caches.get_or_create_concept(conn, &derived.key_folded, name_string_id)?;
        ensure_membership(conn, concept_id, name_string_id, "auto_folded")?;

        obs_stmt.execute(params![
            ingest.run_id,
            source_id,
            name_string_id,
            Country::Fr.as_str(),
            year,
            sex,
            sexe_raw,
            count,
        ])?;
    }

    Ok(())
}

fn ingest_cl(
    ingest: &mut IngestContext,
    source_id: i64,
    path: &Path,
    start_year: i32,
    end_year: i32,
) -> Result<()> {
    if !path.exists() {
        bail!("missing Chile source file: {path:?}");
    }

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b',')
        .from_path(path)
        .with_context(|| format!("open {path:?}"))?;

    let conn = &ingest.conn;
    let caches = &mut ingest.caches;

    let mut obs_stmt = conn.prepare(
        "INSERT INTO name_observation
         (run_id, source_id, name_string_id, country, year, sex, sex_raw, count, proportion)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;

    for result in reader.records() {
        let record = result?;
        let year: i32 = record
            .get(0)
            .context("missing anio")?
            .parse()
            .context("parse year")?;
        if year < start_year || year > end_year {
            continue;
        }
        let name = record.get(1).context("missing nombre")?;
        let sex_raw = record.get(2).context("missing sexo")?;
        let count: i64 = record
            .get(3)
            .context("missing n")?
            .parse()
            .context("parse n")?;
        let proportion: f64 = record
            .get(4)
            .context("missing proporcion")?
            .parse()
            .context("parse proporcion")?;

        let sex = match sex_raw {
            "M" | "F" => sex_raw,
            _ => "U",
        };

        let (name_string_id, derived) = caches.get_or_create_name_string(conn, name)?;
        let concept_id = caches.get_or_create_concept(conn, &derived.key_folded, name_string_id)?;
        ensure_membership(conn, concept_id, name_string_id, "auto_folded")?;

        obs_stmt.execute(params![
            ingest.run_id,
            source_id,
            name_string_id,
            Country::Cl.as_str(),
            year,
            sex,
            sex_raw,
            count,
            proportion,
        ])?;
    }

    Ok(())
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
