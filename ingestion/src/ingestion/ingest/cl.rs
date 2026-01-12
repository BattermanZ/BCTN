use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::params;

use super::{Country, IngestContext, ensure_membership};

pub(super) fn ingest_cl(
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
