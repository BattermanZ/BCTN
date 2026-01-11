use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::params;

use super::{Country, IngestContext, ensure_membership};

pub(super) fn ingest_fr(
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
