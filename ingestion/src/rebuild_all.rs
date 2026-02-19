use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use rusqlite::params;

use crate::cli::{
    IngestArgs, RebuildAllArgs, VariantsBuildArgs, VariantsBuildEnsembleArgs,
    VariantsMaterializeArgs,
};
use crate::db;
use crate::ingestion;

pub(crate) fn run(args: RebuildAllArgs) -> Result<()> {
    if args.start_year > args.end_year {
        bail!("start_year must be <= end_year");
    }
    if !(0.0..=1.0).contains(&args.min_ensemble_score) {
        bail!("min_ensemble_score must be between 0 and 1");
    }

    // 1) Re-ingest names database from sources.
    ingestion::ingest::run(IngestArgs {
        database_source_dir: args.database_source_dir.clone(),
        out: args.out.clone(),
        start_year: args.start_year,
        end_year: args.end_year,
    })?;

    // 2) Wikidata variants build using cache (top N concepts).
    ingestion::variants::run_build(VariantsBuildArgs {
        db: args.out.clone(),
        max_concepts: args.max_concepts,
        request_sleep_ms: args.request_sleep_ms,
        batch_size: args.batch_size,
        cache_path: args.cache_path.clone(),
        refresh_cache: args.refresh_cache,
        no_clear: false,
        progress_every: args.progress_every,
    })?;

    // 3) Ensemble build for the same scope; always clear first to keep idempotence.
    ingestion::variants::run_build_ensemble(VariantsBuildEnsembleArgs {
        db: args.out.clone(),
        country: "ALL".to_string(),
        min_total: 1,
        max_concepts: args.max_concepts,
        max_edit_distance: 2,
        min_years_nonzero: 8,
        // Keep the strictness as-is; we'll auto-accept >= min_ensemble_score below.
        accept_threshold: 0.98,
        propose_threshold: 0.85,
        clear_ensemble: true,
    })?;

    // 4) Auto-accept / ignore ensemble edges by score threshold.
    accept_ensemble_by_score(&args)?;

    // 5) Rebuild all derived tables (groups, totals, spellings, ranks).
    ingestion::variants::run_materialize(VariantsMaterializeArgs {
        db: args.out.clone(),
    })?;

    Ok(())
}

fn accept_ensemble_by_score(args: &RebuildAllArgs) -> Result<()> {
    let mut conn = db::open_rw_with_schema(&args.out)?;
    let tx = conn.transaction()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    tx.execute(
        r#"
        UPDATE name_variant_edge
        SET status = 'accepted', updated_at = ?2
        WHERE method = 'ensemble'
          AND status != 'accepted'
          AND score >= ?1
        "#,
        params![args.min_ensemble_score, now],
    )
    .context("accept ensemble edges")?;

    tx.execute(
        r#"
        UPDATE name_variant_edge
        SET status = 'ignored', updated_at = ?2
        WHERE method = 'ensemble'
          AND status != 'accepted'
          AND score < ?1
        "#,
        params![args.min_ensemble_score, now],
    )
    .context("ignore ensemble edges")?;

    tx.commit()?;
    Ok(())
}
