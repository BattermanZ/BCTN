mod normalize;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

#[derive(Debug, Parser)]
#[command(name = "bctn", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Ingest(IngestArgs),
    Search(SearchArgs),
    Show(ShowArgs),
    Top(TopArgs),
}

#[derive(Debug, Parser)]
struct IngestArgs {
    #[arg(long, default_value = "database_source")]
    database_source_dir: PathBuf,

    #[arg(long, default_value = "data/bctn.sqlite")]
    out: PathBuf,

    #[arg(long, default_value_t = 2005)]
    start_year: i32,

    #[arg(long, default_value_t = 2024)]
    end_year: i32,
}

#[derive(Debug, Parser)]
struct SearchArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    db: PathBuf,

    query: String,

    #[arg(long)]
    contains: bool,

    #[arg(long, default_value_t = 25)]
    limit: u32,
}

#[derive(Debug, Parser)]
struct ShowArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    db: PathBuf,

    term: String,
}

#[derive(Debug, Parser)]
struct TopArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    db: PathBuf,

    #[arg(long)]
    country: String,

    #[arg(long)]
    year: i32,

    #[arg(long)]
    sex: Option<String>,

    #[arg(long, default_value_t = 25)]
    limit: u32,
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
    match cli.command {
        Command::Ingest(args) => run_ingest(args),
        Command::Search(args) => run_search(args),
        Command::Show(args) => run_show(args),
        Command::Top(args) => run_top(args),
    }
}

fn open_readonly(db: &Path) -> Result<Connection> {
    Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("open sqlite db (read-only) {db:?}"))
}

fn fold_query(input: &str) -> Result<String> {
    let norm = normalize::norm_display(input);
    let key_strict = normalize::key_strict(&norm);
    let folded = normalize::key_folded(&key_strict);
    if folded.is_empty() {
        bail!("query folds to empty string");
    }
    Ok(folded)
}

fn run_search(args: SearchArgs) -> Result<()> {
    let conn = open_readonly(&args.db)?;

    let folded = fold_query(&args.query)?;
    let pattern = if args.contains {
        format!("%{folded}%")
    } else {
        format!("{folded}%")
    };
    let mut stmt = conn.prepare(
        "SELECT
           c.concept_key,
           pref.display AS preferred,
           group_concat(DISTINCT o.country) AS locales
         FROM name_concept c
         JOIN name_string pref ON pref.id = c.preferred_name_string_id
         JOIN name_concept_member m ON m.concept_id = c.id
         JOIN name_observation o ON o.name_string_id = m.name_string_id
         WHERE c.concept_key LIKE ?1
         GROUP BY c.id
         ORDER BY pref.display
         LIMIT ?2",
    )?;

    let mut rows = stmt.query(params![pattern, args.limit])?;
    while let Some(row) = rows.next()? {
        let concept_key: String = row.get(0)?;
        let preferred: String = row.get(1)?;
        let locales: String = row.get(2)?;
        println!("{concept_key}\t{preferred}\t{locales}");
    }
    Ok(())
}

fn run_show(args: ShowArgs) -> Result<()> {
    let conn = open_readonly(&args.db)?;

    let folded = fold_query(&args.term)?;
    let concept_id: i64 = conn
        .query_row(
            "SELECT id FROM name_concept WHERE concept_key = ?1",
            params![folded],
            |r| r.get(0),
        )
        .optional()?
        .with_context(|| format!("no concept found for key '{folded}'"))?;

    let (concept_key, preferred): (String, String) = conn.query_row(
        "SELECT c.concept_key, ns.display
         FROM name_concept c
         JOIN name_string ns ON ns.id = c.preferred_name_string_id
         WHERE c.id = ?1",
        params![concept_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    println!("concept_key\t{concept_key}");
    println!("preferred\t{preferred}");

    println!("spellings");
    let mut spellings_stmt = conn.prepare(
        "SELECT DISTINCT o.name_raw
         FROM name_observation o
         JOIN name_concept_member m ON m.name_string_id = o.name_string_id
         WHERE m.concept_id = ?1
         ORDER BY o.name_raw
         LIMIT 200",
    )?;
    let mut spellings = spellings_stmt.query(params![concept_id])?;
    while let Some(row) = spellings.next()? {
        let display: String = row.get(0)?;
        println!("\t{display}");
    }

    println!("by_country\tcountry\tmin_year\tmax_year\ttotal_count");
    let mut country_stmt = conn.prepare(
        "SELECT o.country, MIN(o.year), MAX(o.year), SUM(o.count)
         FROM name_observation o
         JOIN name_concept_member m ON m.name_string_id = o.name_string_id
         WHERE m.concept_id = ?1
         GROUP BY o.country
         ORDER BY o.country",
    )?;
    let mut country_rows = country_stmt.query(params![concept_id])?;
    while let Some(row) = country_rows.next()? {
        let country: String = row.get(0)?;
        let min_year: i32 = row.get(1)?;
        let max_year: i32 = row.get(2)?;
        let total: i64 = row.get(3)?;
        println!("\t{country}\t{min_year}\t{max_year}\t{total}");
    }

    Ok(())
}

fn run_top(args: TopArgs) -> Result<()> {
    let conn = open_readonly(&args.db)?;
    if args.country != "US" && args.country != "FR" && args.country != "CL" {
        bail!("country must be one of: US, FR, CL");
    }
    let sex = args.sex.as_deref();
    if let Some(sex) = sex
        && sex != "M"
        && sex != "F"
        && sex != "U"
    {
        bail!("sex must be one of: M, F, U");
    }

    let mut stmt = conn.prepare(
        "SELECT
           c.concept_key,
           pref.display AS preferred,
           SUM(o.count) AS total
         FROM name_observation o
         JOIN name_concept_member m ON m.name_string_id = o.name_string_id
         JOIN name_concept c ON c.id = m.concept_id
         JOIN name_string pref ON pref.id = c.preferred_name_string_id
         WHERE o.country = ?1
           AND o.year = ?2
           AND (?3 IS NULL OR o.sex = ?3)
         GROUP BY c.id
         ORDER BY total DESC
         LIMIT ?4",
    )?;

    let mut rows = stmt.query(params![args.country, args.year, sex, args.limit])?;
    while let Some(row) = rows.next()? {
        let concept_key: String = row.get(0)?;
        let preferred: String = row.get(1)?;
        let total: i64 = row.get(2)?;
        println!("{concept_key}\t{preferred}\t{total}");
    }
    Ok(())
}

fn run_ingest(args: IngestArgs) -> Result<()> {
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

    ingest_us(
        &mut ingest,
        sources.us_ssa,
        &args.database_source_dir.join("US_names"),
        args.start_year,
        args.end_year,
    )?;
    ingest_fr(
        &mut ingest,
        sources.fr_insee,
        &args.database_source_dir.join("French_names.csv"),
        args.start_year,
        args.end_year,
    )?;
    ingest_cl(
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
        self.conn
            .execute_batch(include_str!("schema_indexes.sql"))?;
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
         (run_id, source_id, name_string_id, name_raw, country, year, sex, sex_raw, count, proportion)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
    )?;

    for path in entries {
        let year = parse_us_year(&path)?;
        if year < start_year || year > end_year {
            continue;
        }

        let file = File::open(&path).with_context(|| format!("open {path:?}"))?;
        let reader = BufReader::new(file);
        for (line_no, line) in reader.lines().enumerate() {
            let line = line.with_context(|| format!("read {path:?}:{line_no}"))?;
            if line.trim().is_empty() {
                continue;
            }
            let (name, sex_raw, count) =
                parse_us_line(&line).with_context(|| format!("parse {path:?}:{line_no}"))?;
            let (name_string_id, derived) = caches.get_or_create_name_string(conn, name)?;
            let concept_id =
                caches.get_or_create_concept(conn, &derived.key_folded, name_string_id)?;
            ensure_membership(conn, concept_id, name_string_id, "auto_folded")?;

            obs_stmt.execute(params![
                ingest.run_id,
                source_id,
                name_string_id,
                name,
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
        .buffer_capacity(1024 * 1024)
        .from_path(path)
        .with_context(|| format!("open {path:?}"))?;

    let conn = &ingest.conn;
    let caches = &mut ingest.caches;

    let mut obs_stmt = conn.prepare(
        "INSERT INTO name_observation
         (run_id, source_id, name_string_id, name_raw, country, year, sex, sex_raw, count, proportion)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
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
            name,
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
        .buffer_capacity(1024 * 1024)
        .from_path(path)
        .with_context(|| format!("open {path:?}"))?;

    let conn = &ingest.conn;
    let caches = &mut ingest.caches;

    let mut obs_stmt = conn.prepare(
        "INSERT INTO name_observation
         (run_id, source_id, name_string_id, name_raw, country, year, sex, sex_raw, count, proportion)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
            name,
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
