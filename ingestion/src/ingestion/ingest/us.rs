use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::params;

use super::{Country, IngestContext, ensure_membership};

pub(super) fn ingest_us(
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
