use std::collections::HashMap;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use rusqlite::{Connection, params};

use crate::cli::VariantsMaterializeArgs;
use crate::db;

#[derive(Debug)]
struct Dsu {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl Dsu {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let root = self.find(self.parent[x]);
            self.parent[x] = root;
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        let rank_a = self.rank[ra];
        let rank_b = self.rank[rb];
        if rank_a < rank_b {
            self.parent[ra] = rb;
        } else if rank_a > rank_b {
            self.parent[rb] = ra;
        } else {
            self.parent[rb] = ra;
            self.rank[ra] = rank_a.saturating_add(1);
        }
    }
}

fn apply_materialize_pragmas(conn: &Connection) {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(8) as i64;

    if let Err(e) = conn.pragma_update(None, "threads", threads) {
        eprintln!("materialize_pragma_failed\tname=threads\terror={e}");
    } else {
        eprintln!("materialize_pragma\tname=threads\tvalue={threads}");
    }

    if let Err(e) = conn.pragma_update(None, "temp_store", "MEMORY") {
        eprintln!("materialize_pragma_failed\tname=temp_store\terror={e}");
    } else {
        eprintln!("materialize_pragma\tname=temp_store\tvalue=MEMORY");
    }

    // Negative cache_size is interpreted as KiB.
    let cache_kib: i64 = -65_536; // ~64MB
    if let Err(e) = conn.pragma_update(None, "cache_size", cache_kib) {
        eprintln!("materialize_pragma_failed\tname=cache_size\terror={e}");
    } else {
        eprintln!(
            "materialize_pragma\tname=cache_size\tvalue_kib={}",
            -cache_kib
        );
    }
}

fn count_rows(conn: &Connection, table: &str) -> Result<i64> {
    let mut stmt = conn.prepare(&format!("SELECT COUNT(*) FROM {table}"))?;
    let count: i64 = stmt.query_row([], |r| r.get(0))?;
    Ok(count)
}

pub(super) fn run_materialize(args: VariantsMaterializeArgs) -> Result<()> {
    let mut conn = db::open_rw_with_schema(&args.db)?;
    db::ensure_indexes(&conn)?;
    apply_materialize_pragmas(&conn);

    let tx = conn.transaction()?;

    let run_id: i64 = tx.query_row("SELECT MAX(id) FROM import_run", [], |r| r.get(0))?;
    eprintln!("materialize_start\trun_id={run_id}");

    let clear_started = Instant::now();
    tx.execute_batch(
        r#"
        DELETE FROM variant_group_country_top100_any;
        DELETE FROM variant_group_country_window_total;
        DELETE FROM concept_country_top100_any;
        DELETE FROM concept_country_window_total;
        DELETE FROM variant_group_member;
        DELETE FROM variant_group;
        DELETE FROM card_rank_last5;
        DELETE FROM card_spelling_subset_top;
        DELETE FROM card_spelling_total_all;
        DELETE FROM card_spelling_country_total;
        "#,
    )?;
    eprintln!(
        "materialize_step\taction=clear_tables\telapsed_ms={}",
        clear_started.elapsed().as_millis()
    );

    let started = Instant::now();
    materialize_groups(&tx)?;
    eprintln!(
        "materialize_step\taction=groups\telapsed_ms={}\tgroups={}\tmembers={}",
        started.elapsed().as_millis(),
        count_rows(&tx, "variant_group")?,
        count_rows(&tx, "variant_group_member")?
    );

    let started = Instant::now();
    materialize_concept_stats(&tx)?;
    eprintln!(
        "materialize_step\taction=concept_stats\telapsed_ms={}\trows={}",
        started.elapsed().as_millis(),
        count_rows(&tx, "concept_country_window_total")?
    );

    let started = Instant::now();
    materialize_group_stats(&tx)?;
    eprintln!(
        "materialize_step\taction=group_stats\telapsed_ms={}\trows={}",
        started.elapsed().as_millis(),
        count_rows(&tx, "variant_group_country_window_total")?
    );

    let started = Instant::now();
    materialize_card_spellings(&tx, run_id)?;
    eprintln!(
        "materialize_step\taction=card_spellings\telapsed_ms={}\tspellings={}\tsubset_top={}",
        started.elapsed().as_millis(),
        count_rows(&tx, "card_spelling_total_all")?,
        count_rows(&tx, "card_spelling_subset_top")?
    );

    let started = Instant::now();
    materialize_card_ranks(&tx, run_id)?;
    eprintln!(
        "materialize_step\taction=card_ranks\telapsed_ms={}\tranks={}",
        started.elapsed().as_millis(),
        count_rows(&tx, "card_rank_last5")?
    );

    let commit_started = Instant::now();
    tx.commit()?;
    eprintln!(
        "materialize_done\trun_id={run_id}\telapsed_ms={}",
        commit_started.elapsed().as_millis()
    );
    Ok(())
}

fn materialize_groups(conn: &Connection) -> Result<()> {
    let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    let mut stmt = conn.prepare("SELECT id, concept_key FROM name_concept ORDER BY id")?;
    let mut rows = stmt.query([])?;
    let mut concept_ids = Vec::<i64>::new();
    let mut concept_keys = Vec::<String>::new();
    let mut id_to_index = HashMap::<i64, usize>::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let key: String = row.get(1)?;
        let idx = concept_ids.len();
        concept_ids.push(id);
        concept_keys.push(key);
        id_to_index.insert(id, idx);
    }

    if concept_ids.is_empty() {
        bail!("no concepts found; ingest must run first");
    }

    let mut dsu = Dsu::new(concept_ids.len());
    let mut edge_stmt = conn.prepare(
        "SELECT from_concept_id, to_concept_id
         FROM name_variant_edge
         WHERE status = 'accepted'",
    )?;
    let mut edge_rows = edge_stmt.query([])?;
    while let Some(row) = edge_rows.next()? {
        let from_id: i64 = row.get(0)?;
        let to_id: i64 = row.get(1)?;
        let Some(&a) = id_to_index.get(&from_id) else {
            continue;
        };
        let Some(&b) = id_to_index.get(&to_id) else {
            continue;
        };
        dsu.union(a, b);
    }

    let mut members_by_root: HashMap<usize, Vec<usize>> = HashMap::new();
    for idx in 0..concept_ids.len() {
        let root = dsu.find(idx);
        members_by_root.entry(root).or_default().push(idx);
    }

    let mut insert_group = conn.prepare(
        "INSERT OR IGNORE INTO variant_group (variant_group_key, created_at) VALUES (?1, ?2)",
    )?;
    let mut insert_member = conn.prepare(
        "INSERT OR IGNORE INTO variant_group_member (variant_group_key, concept_key) VALUES (?1, ?2)",
    )?;

    for members in members_by_root.values() {
        if members.len() < 2 {
            continue;
        }
        let mut min_key = None::<&str>;
        for &idx in members {
            let key = concept_keys[idx].as_str();
            min_key = Some(match min_key {
                None => key,
                Some(existing) => existing.min(key),
            });
        }
        let Some(group_key) = min_key else {
            continue;
        };

        insert_group.execute(params![group_key, started])?;
        for &idx in members {
            insert_member.execute(params![group_key, concept_keys[idx]])?;
        }
    }

    Ok(())
}

fn materialize_concept_stats(conn: &Connection) -> Result<()> {
    let run_id: i64 = conn.query_row("SELECT MAX(id) FROM import_run", [], |r| r.get(0))?;

    conn.execute_batch(
        r#"
        DELETE FROM concept_country_window_total;
        DELETE FROM concept_country_top100_any;
        "#,
    )?;

    conn.execute(
        r#"
        INSERT INTO concept_country_window_total (concept_id, country, total, male, female)
        SELECT
          ncm.concept_id,
          no.country,
          SUM(no.count) AS total,
          SUM(CASE WHEN no.sex = 'M' THEN no.count ELSE 0 END) AS male,
          SUM(CASE WHEN no.sex = 'F' THEN no.count ELSE 0 END) AS female
        FROM name_observation no
        JOIN name_concept_member ncm ON ncm.name_string_id = no.name_string_id
        WHERE no.run_id = ?1
        GROUP BY ncm.concept_id, no.country
        "#,
        params![run_id],
    )?;

    conn.execute(
        r#"
        WITH concept_year AS (
          SELECT
            ncm.concept_id AS concept_id,
            no.country AS country,
            no.year AS year,
            SUM(no.count) AS total
          FROM name_observation no
          JOIN name_concept_member ncm ON ncm.name_string_id = no.name_string_id
          WHERE no.run_id = ?1
          GROUP BY ncm.concept_id, no.country, no.year
        ),
        ranked AS (
          SELECT
            concept_id,
            country,
            year,
            total,
            DENSE_RANK() OVER (PARTITION BY country, year ORDER BY total DESC) AS rnk
          FROM concept_year
        )
        INSERT INTO concept_country_top100_any (concept_id, country, top100_any)
        SELECT concept_id, country, 1
        FROM ranked
        WHERE rnk <= 100
        GROUP BY concept_id, country
        "#,
        params![run_id],
    )?;

    Ok(())
}

fn materialize_group_stats(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        DELETE FROM variant_group_country_window_total;
        DELETE FROM variant_group_country_top100_any;
        "#,
    )?;

    // Totals per group are sums over member concepts.
    conn.execute(
        r#"
        INSERT INTO variant_group_country_window_total (variant_group_key, country, total, male, female)
        SELECT
          vgm.variant_group_key,
          ctot.country,
          SUM(ctot.total) AS total,
          SUM(ctot.male) AS male,
          SUM(ctot.female) AS female
        FROM variant_group_member vgm
        JOIN name_concept nc ON nc.concept_key = vgm.concept_key
        JOIN concept_country_window_total ctot ON ctot.concept_id = nc.id
        GROUP BY vgm.variant_group_key, ctot.country
        "#,
        [],
    )?;

    // top100_any per group is true if any member concept is top100_any.
    conn.execute(
        r#"
        INSERT INTO variant_group_country_top100_any (variant_group_key, country, top100_any)
        SELECT
          vgm.variant_group_key,
          ctop.country,
          1
        FROM variant_group_member vgm
        JOIN name_concept nc ON nc.concept_key = vgm.concept_key
        JOIN concept_country_top100_any ctop ON ctop.concept_id = nc.id
        GROUP BY vgm.variant_group_key, ctop.country
        "#,
        [],
    )?;

    Ok(())
}

fn materialize_card_spellings(conn: &Connection, run_id: i64) -> Result<()> {
    conn.execute_batch(
        r#"
        DELETE FROM card_spelling_country_total;
        DELETE FROM card_spelling_total_all;
        DELETE FROM card_spelling_subset_top;
        "#,
    )?;

    // Per concept spelling totals, per country.
    conn.execute(
        r#"
        INSERT INTO card_spelling_country_total (card_kind, card_key, country, name_raw, total)
        SELECT
          'concept' AS card_kind,
          nc.concept_key AS card_key,
          no.country AS country,
          no.name_raw AS name_raw,
          SUM(no.count) AS total
        FROM name_concept nc
        JOIN name_concept_member ncm ON ncm.concept_id = nc.id
        JOIN name_observation no ON no.name_string_id = ncm.name_string_id
        WHERE no.run_id = ?1 AND no.country IN ('US','FR','CL')
        GROUP BY nc.concept_key, no.country, no.name_raw
        "#,
        params![run_id],
    )?;

    // Per group spelling totals, per country (sum over member concepts).
    conn.execute(
        r#"
        INSERT INTO card_spelling_country_total (card_kind, card_key, country, name_raw, total)
        SELECT
          'group' AS card_kind,
          vgm.variant_group_key AS card_key,
          no.country AS country,
          no.name_raw AS name_raw,
          SUM(no.count) AS total
        FROM variant_group_member vgm
        JOIN name_concept nc ON nc.concept_key = vgm.concept_key
        JOIN name_concept_member ncm ON ncm.concept_id = nc.id
        JOIN name_observation no ON no.name_string_id = ncm.name_string_id
        WHERE no.run_id = ?1 AND no.country IN ('US','FR','CL')
        GROUP BY vgm.variant_group_key, no.country, no.name_raw
        "#,
        params![run_id],
    )?;

    // Global totals over all countries (for fast paging in details view).
    conn.execute(
        r#"
        INSERT INTO card_spelling_total_all (card_kind, card_key, name_raw, total)
        SELECT card_kind, card_key, name_raw, SUM(total) AS total
        FROM card_spelling_country_total
        GROUP BY card_kind, card_key, name_raw
        "#,
        [],
    )?;

    // Top spellings per subset of countries. Only 7 subsets since the set of evidence countries is fixed.
    conn.execute(
        r#"
        WITH subset_totals AS (
          SELECT card_kind, card_key, 'US' AS subset_key, name_raw, SUM(total) AS total
          FROM card_spelling_country_total
          WHERE country = 'US'
          GROUP BY card_kind, card_key, name_raw

          UNION ALL
          SELECT card_kind, card_key, 'FR' AS subset_key, name_raw, SUM(total) AS total
          FROM card_spelling_country_total
          WHERE country = 'FR'
          GROUP BY card_kind, card_key, name_raw

          UNION ALL
          SELECT card_kind, card_key, 'CL' AS subset_key, name_raw, SUM(total) AS total
          FROM card_spelling_country_total
          WHERE country = 'CL'
          GROUP BY card_kind, card_key, name_raw

          UNION ALL
          SELECT card_kind, card_key, 'US,FR' AS subset_key, name_raw, SUM(total) AS total
          FROM card_spelling_country_total
          WHERE country IN ('US','FR')
          GROUP BY card_kind, card_key, name_raw

          UNION ALL
          SELECT card_kind, card_key, 'US,CL' AS subset_key, name_raw, SUM(total) AS total
          FROM card_spelling_country_total
          WHERE country IN ('US','CL')
          GROUP BY card_kind, card_key, name_raw

          UNION ALL
          SELECT card_kind, card_key, 'FR,CL' AS subset_key, name_raw, SUM(total) AS total
          FROM card_spelling_country_total
          WHERE country IN ('FR','CL')
          GROUP BY card_kind, card_key, name_raw

          UNION ALL
          SELECT card_kind, card_key, 'US,FR,CL' AS subset_key, name_raw, SUM(total) AS total
          FROM card_spelling_country_total
          WHERE country IN ('US','FR','CL')
          GROUP BY card_kind, card_key, name_raw
        ),
        ranked AS (
          SELECT
            card_kind,
            card_key,
            subset_key,
            name_raw,
            total,
            ROW_NUMBER() OVER (
              PARTITION BY card_kind, card_key, subset_key
              ORDER BY total DESC, name_raw ASC
            ) AS rn
          FROM subset_totals
        )
        INSERT INTO card_spelling_subset_top (card_kind, card_key, subset_key, rank, name_raw, total)
        SELECT card_kind, card_key, subset_key, rn, name_raw, total
        FROM ranked
        WHERE rn <= 10
        "#,
        [],
    )?;

    Ok(())
}

fn materialize_card_ranks(conn: &Connection, run_id: i64) -> Result<()> {
    conn.execute_batch("DELETE FROM card_rank_last5;")?;

    // Rank for each card is based on the card's top spelling for that country/year (what the user sees on the front).
    // For the year window, each country uses its own latest 5 years available in its dataset.
    conn.execute(
        r#"
        WITH country_years AS (
          SELECT country, year
          FROM (
            SELECT
              country,
              year,
              DENSE_RANK() OVER (PARTITION BY country ORDER BY year DESC) AS yrnk
            FROM (
              SELECT DISTINCT country, year
              FROM name_observation
              WHERE run_id = ?1 AND country IN ('US','FR','CL')
            )
          )
          WHERE yrnk <= 5
        ),
        name_year AS (
          SELECT
            no.country AS country,
            no.year AS year,
            no.name_raw AS name_raw,
            SUM(no.count) AS total
          FROM name_observation no
          JOIN country_years cy ON cy.country = no.country AND cy.year = no.year
          WHERE no.run_id = ?1 AND no.country IN ('US','FR','CL')
          GROUP BY no.country, no.year, no.name_raw
        ),
        ranked_names AS (
          SELECT
            country,
            year,
            name_raw,
            total,
            DENSE_RANK() OVER (PARTITION BY country, year ORDER BY total DESC, name_raw ASC) AS rnk
          FROM name_year
        ),
        card_name_year AS (
          -- Per concept spelling totals per country/year.
          SELECT
            'concept' AS card_kind,
            nc.concept_key AS card_key,
            no.country AS country,
            no.year AS year,
            no.name_raw AS name_raw,
            SUM(no.count) AS total
          FROM name_concept nc
          JOIN name_concept_member ncm ON ncm.concept_id = nc.id
          JOIN name_observation no ON no.name_string_id = ncm.name_string_id
          JOIN country_years cy ON cy.country = no.country AND cy.year = no.year
          WHERE no.run_id = ?1 AND no.country IN ('US','FR','CL')
          GROUP BY nc.concept_key, no.country, no.year, no.name_raw

          UNION ALL

          -- Per group spelling totals per country/year (sum over member concepts).
          SELECT
            'group' AS card_kind,
            vgm.variant_group_key AS card_key,
            no.country AS country,
            no.year AS year,
            no.name_raw AS name_raw,
            SUM(no.count) AS total
          FROM variant_group_member vgm
          JOIN name_concept nc ON nc.concept_key = vgm.concept_key
          JOIN name_concept_member ncm ON ncm.concept_id = nc.id
          JOIN name_observation no ON no.name_string_id = ncm.name_string_id
          JOIN country_years cy ON cy.country = no.country AND cy.year = no.year
          WHERE no.run_id = ?1 AND no.country IN ('US','FR','CL')
          GROUP BY vgm.variant_group_key, no.country, no.year, no.name_raw
        ),
        top_spelling_per_card_year AS (
          SELECT
            card_kind,
            card_key,
            country,
            year,
            name_raw,
            total,
            ROW_NUMBER() OVER (
              PARTITION BY card_kind, card_key, country, year
              ORDER BY total DESC, name_raw ASC
            ) AS rn
          FROM card_name_year
        )
        INSERT INTO card_rank_last5 (card_kind, card_key, country, year, rank, total)
        SELECT
          ts.card_kind,
          ts.card_key,
          ts.country,
          ts.year,
          rn.rnk,
          rn.total
        FROM top_spelling_per_card_year ts
        JOIN ranked_names rn
          ON rn.country = ts.country AND rn.year = ts.year AND rn.name_raw = ts.name_raw
        WHERE ts.rn = 1
        "#,
        params![run_id],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::materialize_card_ranks;
    use crate::db;
    use rusqlite::{Connection, params};

    fn insert_name_string(conn: &Connection, id: i64, display: &str) {
        let norm = display.to_lowercase();
        conn.execute(
            "INSERT INTO name_string (id, display, norm, key_strict, key_folded) VALUES (?1,?2,?3,?4,?5)",
            params![id, display, norm, norm, norm],
        )
        .unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_obs(
        conn: &Connection,
        id: i64,
        run_id: i64,
        name_string_id: i64,
        name_raw: &str,
        country: &str,
        year: i64,
        count: i64,
    ) {
        conn.execute(
            r#"
            INSERT INTO name_observation
              (id, run_id, source_id, name_string_id, name_raw, country, year, sex, sex_raw, count, proportion)
            VALUES
              (?1,  ?2,     1,        ?3,            ?4,       ?5,      ?6,   'M', 'M',    ?7,   NULL)
            "#,
            params![id, run_id, name_string_id, name_raw, country, year, count],
        )
        .unwrap();
    }

    #[test]
    fn materialized_ranks_follow_per_year_top_spelling() {
        let conn = Connection::open_in_memory().unwrap();
        db::ensure_schema(&conn).unwrap();
        db::ensure_indexes(&conn).unwrap();

        conn.execute(
            "INSERT INTO source (id, source_key, name, country, url, license) VALUES (1,'t','t','US',NULL,NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO import_run (id, started_at, finished_at, window_start_year, window_end_year) VALUES (1,0,NULL,2005,2024)",
            [],
        )
        .unwrap();

        insert_name_string(&conn, 1, "A");
        insert_name_string(&conn, 2, "B");
        insert_name_string(&conn, 3, "Z");

        conn.execute(
            "INSERT INTO name_concept (id, concept_key, preferred_name_string_id) VALUES (10,'c1',1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO name_concept (id, concept_key, preferred_name_string_id) VALUES (11,'c2',2)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO name_concept_member (concept_id, name_string_id, relation_type) VALUES (10,1,'raw')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO name_concept_member (concept_id, name_string_id, relation_type) VALUES (10,2,'raw')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO name_concept_member (concept_id, name_string_id, relation_type) VALUES (11,2,'raw')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO variant_group (variant_group_key, created_at) VALUES ('g1',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO variant_group_member (variant_group_key, concept_key) VALUES ('g1','c1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO variant_group_member (variant_group_key, concept_key) VALUES ('g1','c2')",
            [],
        )
        .unwrap();

        // 2023: A is top spelling for the card; B is lower. Global rank among names: Z(1), A(2), B(3).
        insert_obs(&conn, 1, 1, 1, "A", "US", 2023, 100);
        insert_obs(&conn, 2, 1, 2, "B", "US", 2023, 50);
        insert_obs(&conn, 3, 1, 3, "Z", "US", 2023, 200);

        // 2024: B is top spelling for the card. Global rank among names: B(1), Z(2), A(3).
        insert_obs(&conn, 4, 1, 1, "A", "US", 2024, 40);
        insert_obs(&conn, 5, 1, 2, "B", "US", 2024, 120);
        insert_obs(&conn, 6, 1, 3, "Z", "US", 2024, 80);

        materialize_card_ranks(&conn, 1).unwrap();

        let ranks: Vec<(i64, i64)> = conn
            .prepare(
                "SELECT year, rank FROM card_rank_last5 WHERE card_kind='concept' AND card_key='c1' AND country='US' ORDER BY year",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(ranks, vec![(2023, 2), (2024, 1)]);

        let group_ranks: Vec<(i64, i64)> = conn
            .prepare(
                "SELECT year, rank FROM card_rank_last5 WHERE card_kind='group' AND card_key='g1' AND country='US' ORDER BY year",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(group_ranks, vec![(2023, 2), (2024, 1)]);
    }
}
