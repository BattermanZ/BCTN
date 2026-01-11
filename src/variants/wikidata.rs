use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cli::VariantsBuildArgs;
use crate::db;
use crate::normalize;

#[derive(Debug, Clone)]
struct ConceptRow {
    concept_id: i64,
    query_name: String,
    search_term: String,
}

pub(super) fn run_build(args: VariantsBuildArgs) -> Result<()> {
    if args.batch_size == 0 {
        bail!("batch_size must be > 0");
    }
    if args.progress_every == 0 {
        // allowed: disables progress logs
    }

    let mut conn = db::open_rw_with_schema(&args.db)?;

    eprintln!(
        "wikidata_start\tdb={}\tmax_concepts={}\tbatch_size={}\trequest_sleep_ms={}\tprogress_every={}",
        args.db.display(),
        args.max_concepts,
        args.batch_size,
        args.request_sleep_ms,
        args.progress_every
    );

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

    let progress_every = args.progress_every as usize;
    let mut matched_qids: Vec<String> = Vec::new();
    let mut qid_by_query_name: HashMap<String, String> = HashMap::new();
    let total_terms = query_name_to_concepts.len();
    let mut searched_terms: usize = 0;
    let mut search_cache_hits: usize = 0;
    for (query_name, concepts) in &query_name_to_concepts {
        let Some(seed) = concepts.first() else {
            continue;
        };
        searched_terms += 1;
        if cache.search_term_to_qid.contains_key(&seed.search_term) {
            search_cache_hits += 1;
        }
        if let Some(qid) = cache.search_given_name_qid(&agent, &seed.search_term, sleep)? {
            matched_qids.push(qid.clone());
            qid_by_query_name.insert(query_name.clone(), qid);
        }
        if progress_every > 0
            && (searched_terms.is_multiple_of(progress_every) || searched_terms == total_terms)
        {
            eprintln!(
                "wikidata_search_progress\tsearched={}\ttotal={}\tcache_hits={}\tqids_found={}",
                searched_terms,
                total_terms,
                search_cache_hits,
                matched_qids.len()
            );
        }
    }
    matched_qids.sort();
    matched_qids.dedup();

    eprintln!(
        "wikidata_search_done\tsearched={}\ttotal={}\tcache_hits={}\tunique_qids={}",
        searched_terms,
        total_terms,
        search_cache_hits,
        matched_qids.len()
    );

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

    let total_batches = matched_qids.len().div_ceil(args.batch_size as usize).max(1);
    let mut batch_no: usize = 0;
    for batch in matched_qids.chunks(args.batch_size as usize) {
        batch_no += 1;
        let missing_entities = batch
            .iter()
            .filter(|qid| !cache.entities.contains_key(*qid))
            .count();
        if progress_every > 0
            && (batch_no == 1
                || batch_no.is_multiple_of(progress_every)
                || batch_no == total_batches)
        {
            eprintln!(
                "wikidata_getentities_progress\tbatch={}/{}\tqids={}\tmissing={}\tcached={}",
                batch_no,
                total_batches,
                batch.len(),
                missing_entities,
                batch.len() - missing_entities
            );
        }
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
    eprintln!("wikidata_cache_stored\tpath={}", args.cache_path.display());

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

fn build_country_evidence(totals: &ConceptCountryTotals, from: i64, to: i64, qid: &str) -> Value {
    let mut countries: HashMap<String, Value> = HashMap::new();
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
        let tmp = PathBuf::from(path).with_extension("json.tmp");
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
