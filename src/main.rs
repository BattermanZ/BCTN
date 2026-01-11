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
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
    Variants {
        #[command(subcommand)]
        command: VariantsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum VariantsCommand {
    Build(VariantsBuildArgs),
    BuildEnsemble(VariantsBuildEnsembleArgs),
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

#[derive(Debug, Parser)]
struct VariantsBuildArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    db: PathBuf,

    #[arg(long, default_value_t = 1000)]
    max_concepts: u32,

    #[arg(long, default_value_t = 500)]
    request_sleep_ms: u64,

    #[arg(long, default_value_t = 50)]
    batch_size: u32,

    #[arg(long, default_value = "data/wikidata_cache.json")]
    cache_path: PathBuf,

    #[arg(long)]
    refresh_cache: bool,

    #[arg(long)]
    no_clear: bool,
}

#[derive(Debug, Parser)]
struct VariantsBuildEnsembleArgs {
    #[arg(long, default_value = "data/bctn.sqlite")]
    db: PathBuf,

    #[arg(long, default_value = "ALL")]
    country: String,

    #[arg(long, default_value_t = 50)]
    min_total: i64,

    #[arg(long, default_value_t = 20000)]
    max_concepts: u32,

    #[arg(long, default_value_t = 2)]
    max_edit_distance: usize,

    #[arg(long, default_value_t = 8)]
    min_years_nonzero: usize,

    #[arg(long, default_value_t = 0.98)]
    accept_threshold: f64,

    #[arg(long, default_value_t = 0.85)]
    propose_threshold: f64,

    #[arg(long)]
    clear_ensemble: bool,
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
        Command::Variants { command } => match command {
            VariantsCommand::Build(args) => run_variants_build(args),
            VariantsCommand::BuildEnsemble(args) => run_variants_build_ensemble(args),
        },
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

fn run_variants_build(args: VariantsBuildArgs) -> Result<()> {
    if args.batch_size == 0 {
        bail!("batch_size must be > 0");
    }

    let mut conn = Connection::open(&args.db)
        .with_context(|| format!("open sqlite db {db:?}", db = args.db))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.execute_batch(include_str!("schema.sql"))?;
    conn.execute_batch(include_str!("schema_indexes.sql"))?;

    if !args.no_clear {
        conn.execute("DELETE FROM name_variant_edge", [])?;
        conn.execute("DELETE FROM wikidata_concept_match", [])?;
    }

    let concepts = load_top_concepts(&conn, args.max_concepts)?;
    let mut query_name_to_concepts: HashMap<String, Vec<ConceptRow>> = HashMap::new();
    for c in concepts {
        query_name_to_concepts
            .entry(c.query_name.clone())
            .or_default()
            .push(c);
    }

    let concept_country_totals = load_concept_country_totals(&conn)?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(20))
        .timeout_write(std::time::Duration::from_secs(20))
        .build();
    let sleep = std::time::Duration::from_millis(args.request_sleep_ms);

    let mut cache = if args.refresh_cache {
        WikidataCache::default()
    } else {
        WikidataCache::load(&args.cache_path).unwrap_or_default()
    };

    let mut matched_qids: Vec<String> = Vec::new();
    let mut qid_by_query_name: HashMap<String, String> = HashMap::new();
    for (query_name, concepts) in &query_name_to_concepts {
        let Some(seed) = concepts.first() else {
            continue;
        };
        if let Some(qid) = cache.search_given_name_qid(&agent, &seed.search_term, sleep)? {
            matched_qids.push(qid.clone());
            qid_by_query_name.insert(query_name.clone(), qid);
        }
    }
    matched_qids.sort();
    matched_qids.dedup();

    let mut all_matches: Vec<WikidataConceptMatch> = Vec::new();
    for (query_name, qid) in &qid_by_query_name {
        if let Some(concepts) = query_name_to_concepts.get(query_name) {
            for c in concepts {
                all_matches.push(WikidataConceptMatch {
                    concept_id: c.concept_id,
                    qid: qid.clone(),
                    matched_label: c.search_term.clone(),
                    matched_lang: None,
                });
            }
        }
    }

    for batch in matched_qids.chunks(args.batch_size as usize) {
        let entities = cache.get_entities(&agent, batch, sleep)?;
        for (qid, entity) in entities {
            for (lang, label) in entity.labels {
                let qn = normalize::key_strict(&normalize::norm_display(&label));
                if let Some(concepts) = query_name_to_concepts.get(&qn) {
                    for c in concepts {
                        all_matches.push(WikidataConceptMatch {
                            concept_id: c.concept_id,
                            qid: qid.clone(),
                            matched_label: label.clone(),
                            matched_lang: Some(lang.clone()),
                        });
                    }
                }
            }
            for (lang, aliases) in entity.aliases {
                for alias in aliases {
                    let qn = normalize::key_strict(&normalize::norm_display(&alias));
                    if let Some(concepts) = query_name_to_concepts.get(&qn) {
                        for c in concepts {
                            all_matches.push(WikidataConceptMatch {
                                concept_id: c.concept_id,
                                qid: qid.clone(),
                                matched_label: alias.clone(),
                                matched_lang: Some(lang.clone()),
                            });
                        }
                    }
                }
            }
        }
    }

    cache.store(&args.cache_path)?;

    println!(
        "wikidata_matches\tconcepts_considered={}\tmatched_qids={}\tconcept_matches={}",
        query_name_to_concepts.len(),
        matched_qids.len(),
        all_matches.len()
    );

    let tx = conn.transaction()?;

    let mut insert_match = tx.prepare(
        "INSERT OR IGNORE INTO wikidata_concept_match (concept_id, qid, matched_label, matched_label_lang)
         VALUES (?1, ?2, ?3, ?4)",
    )?;

    for m in &all_matches {
        insert_match.execute(params![
            m.concept_id,
            m.qid,
            m.matched_label,
            m.matched_lang
        ])?;
    }

    drop(insert_match);
    let qid_to_concepts = load_qid_to_concepts(&tx)?;
    let created_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    let mut insert_edge = tx.prepare(
        "INSERT INTO name_variant_edge
         (from_concept_id, to_concept_id, method, score, status, evidence_json, created_at, updated_at)
         VALUES (?1, ?2, 'wikidata', 1.0, 'accepted', ?3, ?4, ?4)
         ON CONFLICT(from_concept_id, to_concept_id, method) DO UPDATE SET
           score = excluded.score,
           status = excluded.status,
           evidence_json = excluded.evidence_json,
           updated_at = excluded.updated_at",
    )?;

    let mut edges_written: i64 = 0;
    for (qid, concept_ids) in qid_to_concepts {
        if concept_ids.len() < 2 {
            continue;
        }
        for i in 0..concept_ids.len() {
            for j in (i + 1)..concept_ids.len() {
                let a = concept_ids[i];
                let b = concept_ids[j];
                let (from, to) = if a < b { (a, b) } else { (b, a) };

                let evidence = build_country_evidence(&concept_country_totals, from, to, &qid);
                edges_written +=
                    insert_edge.execute(params![from, to, evidence.to_string(), created_at])?
                        as i64;
            }
        }
    }

    drop(insert_edge);
    tx.commit()?;
    println!("variant_edges_written\t{edges_written}");
    Ok(())
}

fn run_variants_build_ensemble(args: VariantsBuildEnsembleArgs) -> Result<()> {
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

    let mut conn = Connection::open(&args.db)
        .with_context(|| format!("open sqlite db {db:?}", db = args.db))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.execute_batch(include_str!("schema.sql"))?;
    conn.execute_batch(include_str!("schema_indexes.sql"))?;

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
            } else if self.status == "proposed" && status == "proposed" {
                // no-op
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

type YearSeries = HashMap<(i64, i32), i64>;
type SexTotals = HashMap<(i64, String), i64>;
type CountryTotalsAll = HashMap<(i64, String), i64>;

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

fn string_score_from_distance(dist: usize) -> f64 {
    match dist {
        0 => 1.0,
        1 => 0.98,
        2 => 0.93,
        3 => 0.88,
        _ => 0.0,
    }
}

fn trend_correlation_signals(
    series: &YearSeries,
    a: i64,
    b: i64,
    min_years_nonzero: usize,
) -> (f64, Option<f64>, usize, f64, Option<f64>) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut years = 0usize;
    for year in 2005..=2024 {
        let av = series.get(&(a, year)).copied().unwrap_or(0);
        let bv = series.get(&(b, year)).copied().unwrap_or(0);
        if av > 0 && bv > 0 {
            xs.push(av as f64);
            ys.push(bv as f64);
            years += 1;
        }
    }
    if years < min_years_nonzero {
        return (0.0, None, years, 0.0, None);
    }

    let corr = pearson_corr(&xs, &ys).unwrap_or(0.0).clamp(-1.0, 1.0);
    let corr_score = corr.max(0.0);
    let sum_x: f64 = xs.iter().sum();
    let sum_y: f64 = ys.iter().sum();
    let ratio = if sum_x > 0.0 && sum_y > 0.0 {
        Some(sum_x / sum_y)
    } else {
        None
    };
    let magnitude_score = ratio
        .map(|r| (1.0 - (r.ln().abs() / 2.0_f64.ln())).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    (corr_score, Some(corr), years, magnitude_score, ratio)
}

fn pearson_corr(xs: &[f64], ys: &[f64]) -> Option<f64> {
    if xs.len() != ys.len() || xs.is_empty() {
        return None;
    }
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den_x = 0.0;
    let mut den_y = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        num += dx * dy;
        den_x += dx * dx;
        den_y += dy * dy;
    }
    let den = (den_x * den_y).sqrt();
    if den == 0.0 { None } else { Some(num / den) }
}

fn sex_agreement_score(sex: &SexTotals, a: i64, b: i64) -> f64 {
    let am = sex.get(&(a, "M".to_owned())).copied().unwrap_or(0);
    let af = sex.get(&(a, "F".to_owned())).copied().unwrap_or(0);
    let bm = sex.get(&(b, "M".to_owned())).copied().unwrap_or(0);
    let bf = sex.get(&(b, "F".to_owned())).copied().unwrap_or(0);
    let at = (am + af) as f64;
    let bt = (bm + bf) as f64;
    if at == 0.0 || bt == 0.0 {
        return 0.5;
    }
    let pa = (am as f64) / at;
    let pb = (bm as f64) / bt;
    let diff = (pa - pb).abs();
    (1.0 - (diff / 0.25)).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy)]
struct CollisionInfo {
    penalty: f64,
    a_total: i64,
    b_total: i64,
    a_countries_ge_100: i32,
    b_countries_ge_100: i32,
}

fn collision_info(totals: &CountryTotalsAll, a: i64, b: i64, dist: usize) -> CollisionInfo {
    let mut a_countries_ge_100 = 0i32;
    let mut b_countries_ge_100 = 0i32;
    let mut a_total = 0i64;
    let mut b_total = 0i64;
    for country in ["US", "FR", "CL"] {
        let av = totals.get(&(a, country.to_owned())).copied().unwrap_or(0);
        let bv = totals.get(&(b, country.to_owned())).copied().unwrap_or(0);
        a_total += av;
        b_total += bv;
        if av >= 100 {
            a_countries_ge_100 += 1;
        }
        if bv >= 100 {
            b_countries_ge_100 += 1;
        }
    }
    let mut penalty = 0.0;
    if (a_countries_ge_100 - b_countries_ge_100).abs() >= 2 {
        penalty += 0.05;
    }
    if dist == 1 && a_total >= 50_000 && b_total >= 50_000 {
        penalty += 0.10;
    }
    CollisionInfo {
        penalty,
        a_total,
        b_total,
        a_countries_ge_100,
        b_countries_ge_100,
    }
}

#[derive(Debug, Default, Clone)]
struct ConceptQids {
    primary_qid: Option<String>,
    primary_conflict: bool,
    all_qids: Vec<String>,
}

type ConceptQidsByConcept = HashMap<i64, ConceptQids>;

fn load_country_concept_qids(conn: &Connection) -> Result<ConceptQidsByConcept> {
    let mut stmt = conn.prepare(
        "SELECT tc.id, w.qid, w.matched_label_lang
         FROM tmp_concept tc
         JOIN wikidata_concept_match w ON w.concept_id = tc.id",
    )?;
    let mut rows = stmt.query([])?;
    let mut out: ConceptQidsByConcept = HashMap::new();
    while let Some(row) = rows.next()? {
        let concept_id: i64 = row.get(0)?;
        let qid: String = row.get(1)?;
        let matched_label_lang: Option<String> = row.get(2)?;

        let entry = out.entry(concept_id).or_default();
        entry.all_qids.push(qid.clone());
        if matched_label_lang.is_none() {
            // Seed lookup match (most reliable): treat as primary QID.
            if entry.primary_conflict {
                continue;
            }
            match &entry.primary_qid {
                None => entry.primary_qid = Some(qid),
                Some(existing) => {
                    if existing != &qid {
                        // Multiple "primary" QIDs for one concept is ambiguous; mark conflict.
                        entry.primary_qid = None;
                        entry.primary_conflict = true;
                    }
                }
            }
        }
    }
    Ok(out)
}

fn concepts_share_any_qid_from_map(qids: &ConceptQidsByConcept, a: i64, b: i64) -> bool {
    let Some(aq) = qids.get(&a) else { return false };
    let Some(bq) = qids.get(&b) else { return false };
    if aq.all_qids.len() <= bq.all_qids.len() {
        for q in &aq.all_qids {
            if bq.all_qids.iter().any(|x| x == q) {
                return true;
            }
        }
    } else {
        for q in &bq.all_qids {
            if aq.all_qids.iter().any(|x| x == q) {
                return true;
            }
        }
    }
    false
}

fn concepts_share_primary_qid_from_map(qids: &ConceptQidsByConcept, a: i64, b: i64) -> bool {
    let Some(aq) = qids.get(&a) else { return false };
    let Some(bq) = qids.get(&b) else { return false };
    match (aq.primary_qid.as_deref(), bq.primary_qid.as_deref()) {
        (Some(qa), Some(qb)) => qa == qb,
        _ => false,
    }
}

fn levenshtein_bounded(a: &str, b: &str, max: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let len_a = a.len();
    let len_b = b.len();
    if len_a.abs_diff(len_b) > max {
        return None;
    }
    if len_a == 0 {
        return (len_b <= max).then_some(len_b);
    }
    if len_b == 0 {
        return (len_a <= max).then_some(len_a);
    }

    let mut prev: Vec<usize> = (0..=len_b).collect();
    let mut curr: Vec<usize> = vec![0; len_b + 1];

    for i in 1..=len_a {
        curr[0] = i;
        let mut row_min = curr[0];
        for j in 1..=len_b {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let del = prev[j] + 1;
            let ins = curr[j - 1] + 1;
            let sub = prev[j - 1] + cost;
            let v = del.min(ins).min(sub);
            curr[j] = v;
            row_min = row_min.min(v);
        }
        if row_min > max {
            return None;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    let dist = prev[len_b];
    (dist <= max).then_some(dist)
}

#[derive(Debug, Clone)]
struct ConceptRow {
    concept_id: i64,
    query_name: String,
    search_term: String,
}

fn load_top_concepts(conn: &Connection, max_concepts: u32) -> Result<Vec<ConceptRow>> {
    let mut stmt = conn.prepare(
        "SELECT
           c.id,
           LOWER(ns.norm) AS query_name,
           ns.norm AS search_term,
           SUM(o.count) AS total
         FROM name_concept c
         JOIN name_string ns ON ns.id = c.preferred_name_string_id
         JOIN name_concept_member m ON m.concept_id = c.id
         JOIN name_observation o ON o.name_string_id = m.name_string_id
         GROUP BY c.id
         ORDER BY total DESC
         LIMIT ?1",
    )?;

    let mut rows = stmt.query(params![max_concepts])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let concept_id: i64 = row.get(0)?;
        let query_name: String = row.get(1)?;
        let search_term: String = row.get(2)?;
        out.push(ConceptRow {
            concept_id,
            query_name,
            search_term,
        });
    }
    Ok(out)
}

type ConceptCountryTotals = HashMap<(i64, String), i64>;

fn load_concept_country_totals(conn: &Connection) -> Result<ConceptCountryTotals> {
    let mut stmt = conn.prepare(
        "SELECT c.id, o.country, SUM(o.count)
         FROM name_concept c
         JOIN name_concept_member m ON m.concept_id = c.id
         JOIN name_observation o ON o.name_string_id = m.name_string_id
         GROUP BY c.id, o.country",
    )?;

    let mut rows = stmt.query([])?;
    let mut out: ConceptCountryTotals = HashMap::new();
    while let Some(row) = rows.next()? {
        let concept_id: i64 = row.get(0)?;
        let country: String = row.get(1)?;
        let total: i64 = row.get(2)?;
        out.insert((concept_id, country), total);
    }
    Ok(out)
}

fn build_country_evidence(
    totals: &ConceptCountryTotals,
    from: i64,
    to: i64,
    qid: &str,
) -> serde_json::Value {
    let mut countries: HashMap<String, serde_json::Value> = HashMap::new();
    for country in ["US", "FR", "CL"] {
        let a = totals
            .get(&(from, country.to_owned()))
            .copied()
            .unwrap_or(0);
        let b = totals.get(&(to, country.to_owned())).copied().unwrap_or(0);
        if a == 0 && b == 0 {
            continue;
        }
        countries.insert(country.to_owned(), json!({"from_total": a, "to_total": b}));
    }
    json!({
        "wikidata": { "qid": qid },
        "countries": countries
    })
}

fn load_qid_to_concepts(conn: &Connection) -> Result<HashMap<String, Vec<i64>>> {
    let mut stmt = conn.prepare("SELECT DISTINCT qid, concept_id FROM wikidata_concept_match")?;
    let mut rows = stmt.query([])?;
    let mut out: HashMap<String, Vec<i64>> = HashMap::new();
    while let Some(row) = rows.next()? {
        let qid: String = row.get(0)?;
        let concept_id: i64 = row.get(1)?;
        out.entry(qid).or_default().push(concept_id);
    }
    for ids in out.values_mut() {
        ids.sort();
        ids.dedup();
    }
    Ok(out)
}

#[cfg(test)]
mod ensemble_tests {
    use super::*;

    #[test]
    fn levenshtein_bounded_basic_cases() {
        assert_eq!(levenshtein_bounded("AYMERIC", "AYMERIC", 2), Some(0));
        assert_eq!(levenshtein_bounded("kitten", "sitting", 2), None);
        assert_eq!(levenshtein_bounded("kitten", "sitting", 3), Some(3));
        assert_eq!(levenshtein_bounded("", "abc", 2), None);
        assert_eq!(levenshtein_bounded("", "abc", 3), Some(3));
    }

    #[test]
    fn pearson_corr_edge_cases() {
        let corr = pearson_corr(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]).unwrap();
        assert!((corr - 1.0).abs() < 1e-12);

        let corr = pearson_corr(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]).unwrap();
        assert!((corr + 1.0).abs() < 1e-12);

        assert_eq!(pearson_corr(&[1.0, 1.0, 1.0], &[2.0, 3.0, 4.0]), None);
        assert_eq!(pearson_corr(&[], &[]), None);
        assert_eq!(pearson_corr(&[1.0], &[1.0, 2.0]), None);
    }
}

#[derive(Debug)]
struct WikidataConceptMatch {
    concept_id: i64,
    qid: String,
    matched_label: String,
    matched_lang: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WbSearchEntitiesResponse {
    #[serde(default)]
    search: Vec<WbSearchEntitiesResult>,
}

#[derive(Debug, Deserialize)]
struct WbSearchEntitiesResult {
    id: String,
    #[serde(default)]
    description: String,
}

fn wikidata_search_given_name_qid(
    agent: &ureq::Agent,
    term: &str,
    sleep: std::time::Duration,
) -> Result<Option<String>> {
    let term = term.trim();
    if term.is_empty() {
        return Ok(None);
    }
    for attempt in 1..=5 {
        std::thread::sleep(sleep);

        let resp = agent
            .get("https://www.wikidata.org/w/api.php")
            .set("User-Agent", "bctn/0.1 (personal project)")
            .query("action", "wbsearchentities")
            .query("search", term)
            .query("language", "en")
            .query("format", "json")
            .query("limit", "5")
            .call();

        match resp {
            Ok(resp) => {
                let body = resp.into_string().context("read wikidata search body")?;
                let parsed: WbSearchEntitiesResponse =
                    serde_json::from_str(&body).context("parse wikidata search json")?;

                for r in parsed.search {
                    let d = r.description.to_lowercase();
                    if d.contains("given name") || d.contains("prénom") || d.contains("nombre") {
                        return Ok(Some(r.id));
                    }
                }
                return Ok(None);
            }
            Err(ureq::Error::Status(429, resp)) => {
                let retry_after = resp
                    .header("Retry-After")
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(10);
                std::thread::sleep(std::time::Duration::from_secs(retry_after));
            }
            Err(ureq::Error::Status(code, _)) if (500..600).contains(&code) && attempt < 5 => {
                std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
            }
            Err(e) => return Err(anyhow::anyhow!("wikidata search failed: {e}")),
        }
    }
    Ok(None)
}

#[derive(Debug, Deserialize)]
struct WbGetEntitiesResponse {
    entities: HashMap<String, WbEntity>,
}

#[derive(Debug, Deserialize)]
struct WbEntity {
    #[serde(default)]
    labels: HashMap<String, WbValue>,
    #[serde(default)]
    aliases: HashMap<String, Vec<WbValue>>,
}

#[derive(Debug, Deserialize)]
struct WbValue {
    value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct WikidataEntityStrings {
    #[serde(default)]
    labels: HashMap<String, String>,
    #[serde(default)]
    aliases: HashMap<String, Vec<String>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct WikidataCache {
    #[serde(default)]
    search_term_to_qid: HashMap<String, Option<String>>,
    #[serde(default)]
    entities: HashMap<String, WikidataEntityStrings>,
}

impl WikidataCache {
    fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).with_context(|| format!("read cache {path:?}"))?;
        serde_json::from_slice(&bytes).context("parse cache json")
    }

    fn store(&self, path: &Path) -> Result<()> {
        let Some(parent) = path.parent() else {
            bail!("cache_path must have a parent directory");
        };
        fs::create_dir_all(parent).with_context(|| format!("create cache dir {parent:?}"))?;
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).context("serialize cache json")?;
        fs::write(&tmp, bytes).with_context(|| format!("write cache tmp {tmp:?}"))?;
        fs::rename(&tmp, path).with_context(|| format!("rename cache {tmp:?} -> {path:?}"))?;
        Ok(())
    }

    fn search_given_name_qid(
        &mut self,
        agent: &ureq::Agent,
        term: &str,
        sleep: std::time::Duration,
    ) -> Result<Option<String>> {
        if let Some(v) = self.search_term_to_qid.get(term) {
            return Ok(v.clone());
        }
        let qid = wikidata_search_given_name_qid(agent, term, sleep)?;
        self.search_term_to_qid.insert(term.to_owned(), qid.clone());
        Ok(qid)
    }

    fn get_entities(
        &mut self,
        agent: &ureq::Agent,
        qids: &[String],
        sleep: std::time::Duration,
    ) -> Result<HashMap<String, WikidataEntityStrings>> {
        let mut missing: Vec<String> = Vec::new();
        for qid in qids {
            if !self.entities.contains_key(qid) {
                missing.push(qid.clone());
            }
        }

        if !missing.is_empty() {
            let fetched = wikidata_get_entities_uncached(agent, &missing, sleep)?;
            for (qid, ent) in fetched {
                self.entities.insert(qid, ent);
            }
        }

        let mut out = HashMap::new();
        for qid in qids {
            if let Some(ent) = self.entities.get(qid).cloned() {
                out.insert(qid.clone(), ent);
            }
        }
        Ok(out)
    }
}

fn wikidata_get_entities_uncached(
    agent: &ureq::Agent,
    qids: &[String],
    sleep: std::time::Duration,
) -> Result<HashMap<String, WikidataEntityStrings>> {
    std::thread::sleep(sleep);
    if qids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids = qids.join("|");
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 1..=3 {
        let resp = agent
            .get("https://www.wikidata.org/w/api.php")
            .set("User-Agent", "bctn/0.1 (personal project)")
            .query("action", "wbgetentities")
            .query("ids", &ids)
            .query("props", "labels|aliases")
            .query("languages", "en|fr|es|mul")
            .query("format", "json")
            .call();

        match resp {
            Ok(resp) => {
                let body = resp
                    .into_string()
                    .context("read wikidata getentities body")?;
                let parsed: WbGetEntitiesResponse =
                    serde_json::from_str(&body).context("parse wikidata getentities json")?;

                let mut out = HashMap::new();
                for (qid, ent) in parsed.entities {
                    let labels = ent
                        .labels
                        .into_iter()
                        .map(|(lang, v)| (lang, v.value))
                        .collect();
                    let aliases = ent
                        .aliases
                        .into_iter()
                        .map(|(lang, vs)| (lang, vs.into_iter().map(|v| v.value).collect()))
                        .collect();
                    out.insert(qid, WikidataEntityStrings { labels, aliases });
                }
                return Ok(out);
            }
            Err(ureq::Error::Status(429, resp)) => {
                let retry_after = resp
                    .header("Retry-After")
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(10);
                std::thread::sleep(std::time::Duration::from_secs(retry_after));
            }
            Err(e) => {
                last_err = Some(anyhow::anyhow!("wikidata getentities failed: {e}"));
                std::thread::sleep(std::time::Duration::from_secs(attempt));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("wikidata getentities failed")))
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
