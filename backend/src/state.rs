use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Row, Sqlite};
use tokio::sync::Mutex;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: Pool<Sqlite>,
    pub(crate) names_db: Pool<Sqlite>,
    pub(crate) country_total_births: CountryTotals,
    pub(crate) cookie_secure: bool,
    pub(crate) login_limiter: Arc<LoginLimiter>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CountryTotals {
    pub(crate) us_total: i64,
    pub(crate) fr_total: i64,
    pub(crate) cl_total: i64,
}

#[derive(Debug)]
pub(crate) struct LoginLimiter {
    inner: Mutex<std::collections::HashMap<String, Vec<i64>>>,
    max_attempts: usize,
    window_seconds: i64,
}

impl LoginLimiter {
    pub(crate) fn new(max_attempts: usize, window_seconds: i64) -> Self {
        Self {
            inner: Mutex::new(std::collections::HashMap::new()),
            max_attempts,
            window_seconds,
        }
    }

    pub(crate) async fn record_and_is_limited(&self, key: &str, now: i64) -> bool {
        let mut m = self.inner.lock().await;
        let cutoff = now - self.window_seconds;
        let v = m.entry(key.to_string()).or_default();
        v.retain(|ts| *ts >= cutoff);
        v.push(now);
        v.len() > self.max_attempts
    }
}

impl AppState {
    pub(crate) async fn new(app_db: &Path, names_db: &Path, cookie_secure: bool) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(app_db)
            .create_if_missing(true)
            .foreign_keys(true);

        let db = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
            .with_context(|| format!("open app db {app_db:?}"))?;

        let names_opts = SqliteConnectOptions::new()
            .filename(names_db)
            .read_only(true)
            .immutable(true)
            .foreign_keys(false);
        let names_db = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(names_opts)
            .await
            .with_context(|| format!("open names db {names_db:?}"))?;

        let run_id = load_latest_run_id(&names_db).await?;
        let country_total_births = load_country_totals(&names_db, run_id).await?;

        Ok(Self {
            db,
            names_db,
            country_total_births,
            cookie_secure,
            login_limiter: Arc::new(LoginLimiter::new(20, 10 * 60)),
        })
    }

    pub(crate) async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.db)
            .await
            .context("run migrations")?;
        Ok(())
    }
}

async fn load_latest_run_id(names_db: &Pool<Sqlite>) -> Result<i64> {
    let row = sqlx::query("SELECT id FROM import_run ORDER BY id DESC LIMIT 1")
        .fetch_one(names_db)
        .await
        .context("load latest import_run id")?;
    Ok(row.try_get("id")?)
}

async fn load_country_totals(names_db: &Pool<Sqlite>, run_id: i64) -> Result<CountryTotals> {
    async fn total_for(names_db: &Pool<Sqlite>, run_id: i64, country: &str) -> Result<i64> {
        let v: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(count), 0) FROM name_observation WHERE run_id = ?1 AND country = ?2",
        )
        .bind(run_id)
        .bind(country)
        .fetch_one(names_db)
        .await
        .with_context(|| format!("sum births for country {country}"))?;
        Ok(v)
    }

    Ok(CountryTotals {
        us_total: total_for(names_db, run_id, "US").await?,
        fr_total: total_for(names_db, run_id, "FR").await?,
        cl_total: total_for(names_db, run_id, "CL").await?,
    })
}
