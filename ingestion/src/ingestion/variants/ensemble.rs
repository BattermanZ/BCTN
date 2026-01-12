use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use anyhow::bail;
use rusqlite::{Connection, params};
use serde_json::{Value, json};

use super::ensemble_score::{
    CountryTotalsAll, SexTotals, YearSeries, collision_info, concepts_share_any_qid_from_map,
    concepts_share_primary_qid_from_map, levenshtein_bounded, load_country_concept_qids,
    sex_agreement_score, string_score_from_distance, trend_correlation_signals,
};
use crate::cli::VariantsBuildEnsembleArgs;
use crate::db;

pub(super) fn run_build_ensemble(args: VariantsBuildEnsembleArgs) -> Result<()> {
    if args.max_edit_distance == 0 {
        bail!("max_edit_distance must be > 0");
    }
    if !(0.0..=1.0).contains(&args.accept_threshold)
        || !(0.0..=1.0).contains(&args.propose_threshold)
    {
        bail!("thresholds must be between 0 and 1");
    }
    if args.propose_threshold > args.accept_threshold {
        bail!("propose_threshold must be <= accept_threshold");
    }

    let mut conn = db::open_rw_with_schema(&args.db)?;

    let countries: Vec<&str> = match args.country.as_str() {
        "ALL" => vec!["US", "FR", "CL"],
        "US" => vec!["US"],
        "FR" => vec!["FR"],
        "CL" => vec!["CL"],
        other => bail!("unknown country {other:?}; expected ALL|US|FR|CL"),
    };

    let tx = conn.transaction()?;
    if args.clear_ensemble {
        tx.execute(
            "DELETE FROM name_variant_edge WHERE method = 'ensemble'",
            [],
        )?;
    }

    #[derive(Debug, Clone)]
    struct EdgeAggregate {
        score: f64,
        status: &'static str,
        shared_qid: bool,
        countries: serde_json::Map<String, serde_json::Value>,
    }

    impl EdgeAggregate {
        fn new() -> Self {
            Self {
                score: 0.0,
                status: "proposed",
                shared_qid: false,
                countries: serde_json::Map::new(),
            }
        }

        fn update(&mut self, country: &str, score: f64, status: &'static str, evidence: Value) {
            if self.shared_qid {
                self.countries.insert(country.to_owned(), evidence);
                return;
            }

            self.score = self.score.max(score);
            if self.status != "accepted" && status == "accepted" {
                self.status = "accepted";
            }
            self.countries.insert(country.to_owned(), evidence);
        }

        fn update_shared_qid(&mut self, country: &str, evidence: Value) {
            self.shared_qid = true;
            self.score = 1.0;
            self.status = "accepted";
            self.countries.insert(country.to_owned(), evidence);
        }

        fn to_evidence_json(&self) -> Value {
            json!({
                "wikidata": {"shared_qid": self.shared_qid},
                "countries": self.countries,
            })
        }
    }

    let mut aggregates: HashMap<(i64, i64), EdgeAggregate> = HashMap::new();

    for country in countries {
        let concepts = load_country_concepts(&tx, country, args.min_total, args.max_concepts)?;
        println!(
            "ensemble_candidates_loaded\tcountry={country}\tconcepts={}",
            concepts.len()
        );

        if concepts.len() < 2 {
            continue;
        }

        tx.execute_batch("DROP TABLE IF EXISTS tmp_concept")?;
        tx.execute_batch("CREATE TEMP TABLE tmp_concept (id INTEGER PRIMARY KEY)")?;
        {
            let mut ins = tx.prepare("INSERT INTO tmp_concept(id) VALUES (?1)")?;
            for c in &concepts {
                ins.execute(params![c.concept_id])?;
            }
        }

        let series = load_country_year_series(&tx, country)?;
        let sex = load_country_sex_totals(&tx, country)?;
        let global_country_totals = load_country_totals_all_countries(&tx)?;
        let qids = load_country_concept_qids(&tx)?;

        let blocks = build_blocks_by_prefix3_sorted_by_len(&concepts);

        for ids in blocks.values() {
            if ids.len() < 2 {
                continue;
            }
            for pos_i in 0..ids.len() {
                let a = &concepts[ids[pos_i]];
                let a_len = a.concept_key.chars().count();
                for pos_j in (pos_i + 1)..ids.len() {
                    let b = &concepts[ids[pos_j]];
                    let b_len = b.concept_key.chars().count();
                    if b_len.saturating_sub(a_len) > args.max_edit_distance {
                        break;
                    }

                    let Some(dist) =
                        levenshtein_bounded(&a.concept_key, &b.concept_key, args.max_edit_distance)
                    else {
                        continue;
                    };

                    let string_score = string_score_from_distance(dist);
                    if string_score < 0.80 {
                        continue;
                    }

                    let shared_primary_qid =
                        concepts_share_primary_qid_from_map(&qids, a.concept_id, b.concept_id);
                    if shared_primary_qid {
                        let evidence = json!({
                            "string": {"a": a.concept_key, "b": b.concept_key, "edit_distance": dist},
                            "wikidata": {"shared_qid": true, "shared_primary_qid": true}
                        });
                        let key = (
                            a.concept_id.min(b.concept_id),
                            a.concept_id.max(b.concept_id),
                        );
                        aggregates
                            .entry(key)
                            .or_insert_with(EdgeAggregate::new)
                            .update_shared_qid(country, evidence);
                        continue;
                    }

                    let (corr_score, corr_opt, years_overlap, magnitude_score, magnitude_ratio) =
                        trend_correlation_signals(
                            &series,
                            a.concept_id,
                            b.concept_id,
                            args.min_years_nonzero,
                        );
                    let sex_score = sex_agreement_score(&sex, a.concept_id, b.concept_id);
                    let collision =
                        collision_info(&global_country_totals, a.concept_id, b.concept_id, dist);

                    let mut ensemble = 0.40 * string_score
                        + 0.30 * corr_score
                        + 0.15 * magnitude_score
                        + 0.15 * sex_score;
                    ensemble = (ensemble - collision.penalty).clamp(0.0, 1.0);

                    let extremely_common_one_edit =
                        dist == 1 && collision.a_total >= 50_000 && collision.b_total >= 50_000;
                    let shared_any_qid =
                        concepts_share_any_qid_from_map(&qids, a.concept_id, b.concept_id);

                    let status = if ensemble >= args.accept_threshold
                        && corr_score >= 0.95
                        && magnitude_score >= 0.90
                        && string_score >= 0.93
                        && sex_score >= 0.80
                        && !extremely_common_one_edit
                    {
                        "accepted"
                    } else if ensemble >= args.propose_threshold {
                        "proposed"
                    } else {
                        continue;
                    };

                    let evidence = json!({
                        "decision": {"status": status, "score": ensemble},
                        "string": {"a": a.concept_key, "b": b.concept_key, "edit_distance": dist, "string_score": string_score},
                        "wikidata": {"shared_qid": shared_any_qid, "shared_primary_qid": false},
                        "trend": {"corr": corr_opt, "years_overlap": years_overlap, "corr_score": corr_score, "magnitude_score": magnitude_score, "magnitude_ratio": magnitude_ratio},
                        "sex": {"sex_score": sex_score},
                        "collision": {"penalty": collision.penalty, "a_total": collision.a_total, "b_total": collision.b_total, "a_countries_ge_100": collision.a_countries_ge_100, "b_countries_ge_100": collision.b_countries_ge_100},
                        "names": {"a_preferred": a.preferred, "b_preferred": b.preferred}
                    });

                    let key = (
                        a.concept_id.min(b.concept_id),
                        a.concept_id.max(b.concept_id),
                    );
                    aggregates
                        .entry(key)
                        .or_insert_with(EdgeAggregate::new)
                        .update(country, ensemble, status, evidence);
                }
            }
        }
    }

    let mut upsert = tx.prepare(
        "INSERT INTO name_variant_edge
         (from_concept_id, to_concept_id, method, score, status, evidence_json, created_at, updated_at)
         VALUES (?1, ?2, 'ensemble', ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(from_concept_id, to_concept_id, method) DO UPDATE SET
           score = excluded.score,
           status = excluded.status,
           evidence_json = excluded.evidence_json,
           updated_at = excluded.updated_at",
    )?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let mut edges_written = 0i64;
    for ((from, to), agg) in aggregates {
        let evidence_json = agg.to_evidence_json();
        edges_written += upsert.execute(params![
            from,
            to,
            agg.score,
            agg.status,
            evidence_json.to_string(),
            now
        ])? as i64;
    }

    drop(upsert);
    println!("ensemble_edges_written\tcount={edges_written}");

    tx.commit()?;
    Ok(())
}

#[derive(Debug, Clone)]
struct CountryConcept {
    concept_id: i64,
    concept_key: String,
    preferred: String,
}

fn load_country_concepts(
    conn: &Connection,
    country: &str,
    min_total: i64,
    max_concepts: u32,
) -> Result<Vec<CountryConcept>> {
    let mut stmt = conn.prepare(
        "SELECT
           c.id,
           c.concept_key,
           ns.display,
           SUM(o.count) AS total
         FROM name_concept c
         JOIN name_string ns ON ns.id = c.preferred_name_string_id
         JOIN name_concept_member m ON m.concept_id = c.id
         JOIN name_observation o ON o.name_string_id = m.name_string_id
         WHERE o.country = ?1
         GROUP BY c.id
         HAVING total >= ?2
         ORDER BY total DESC
         LIMIT ?3",
    )?;

    let mut rows = stmt.query(params![country, min_total, max_concepts])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(CountryConcept {
            concept_id: row.get(0)?,
            concept_key: row.get(1)?,
            preferred: row.get(2)?,
        });
    }
    Ok(out)
}

fn load_country_year_series(conn: &Connection, country: &str) -> Result<YearSeries> {
    let mut stmt = conn.prepare(
        "SELECT tc.id, o.year, SUM(o.count)
         FROM tmp_concept tc
         JOIN name_concept_member m ON m.concept_id = tc.id
         JOIN name_observation o ON o.name_string_id = m.name_string_id
         WHERE o.country = ?1
         GROUP BY tc.id, o.year",
    )?;
    let mut rows = stmt.query(params![country])?;
    let mut out = YearSeries::new();
    while let Some(row) = rows.next()? {
        let concept_id: i64 = row.get(0)?;
        let year: i32 = row.get(1)?;
        let total: i64 = row.get(2)?;
        out.insert((concept_id, year), total);
    }
    Ok(out)
}

fn load_country_sex_totals(conn: &Connection, country: &str) -> Result<SexTotals> {
    let mut stmt = conn.prepare(
        "SELECT tc.id, o.sex, SUM(o.count)
         FROM tmp_concept tc
         JOIN name_concept_member m ON m.concept_id = tc.id
         JOIN name_observation o ON o.name_string_id = m.name_string_id
         WHERE o.country = ?1
         GROUP BY tc.id, o.sex",
    )?;
    let mut rows = stmt.query(params![country])?;
    let mut out = SexTotals::new();
    while let Some(row) = rows.next()? {
        let concept_id: i64 = row.get(0)?;
        let sex: String = row.get(1)?;
        let total: i64 = row.get(2)?;
        out.insert((concept_id, sex), total);
    }
    Ok(out)
}

fn load_country_totals_all_countries(conn: &Connection) -> Result<CountryTotalsAll> {
    let mut stmt = conn.prepare(
        "SELECT tc.id, o.country, SUM(o.count)
         FROM tmp_concept tc
         JOIN name_concept_member m ON m.concept_id = tc.id
         JOIN name_observation o ON o.name_string_id = m.name_string_id
         GROUP BY tc.id, o.country",
    )?;
    let mut rows = stmt.query([])?;
    let mut out = CountryTotalsAll::new();
    while let Some(row) = rows.next()? {
        let concept_id: i64 = row.get(0)?;
        let country: String = row.get(1)?;
        let total: i64 = row.get(2)?;
        out.insert((concept_id, country), total);
    }
    Ok(out)
}

fn build_blocks_by_prefix3_sorted_by_len(
    concepts: &[CountryConcept],
) -> HashMap<String, Vec<usize>> {
    let mut blocks: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, c) in concepts.iter().enumerate() {
        let prefix: String = c.concept_key.chars().take(3).collect();
        blocks.entry(prefix).or_default().push(idx);
    }
    for ids in blocks.values_mut() {
        ids.sort_by_key(|idx| concepts[*idx].concept_key.chars().count());
    }
    blocks
}
