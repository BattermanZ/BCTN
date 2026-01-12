use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, params};

use crate::cli::{SearchArgs, ShowArgs, TopArgs};
use crate::db;
use crate::normalize;

fn fold_query(input: &str) -> Result<String> {
    let norm = normalize::norm_display(input);
    let key_strict = normalize::key_strict(&norm);
    let folded = normalize::key_folded(&key_strict);
    if folded.is_empty() {
        bail!("query folds to empty string");
    }
    Ok(folded)
}

pub(crate) fn run_search(args: SearchArgs) -> Result<()> {
    let conn = db::open_readonly(&args.db)?;

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

pub(crate) fn run_show(args: ShowArgs) -> Result<()> {
    let conn = db::open_readonly(&args.db)?;

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

pub(crate) fn run_top(args: TopArgs) -> Result<()> {
    let conn = db::open_readonly(&args.db)?;
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
