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
    pub(crate) public_host: Option<String>,
    pub(crate) trust_proxy_headers: bool,
    pub(crate) proxy_header_secret: Option<String>,
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
    max_keys: usize,
}

impl LoginLimiter {
    pub(crate) fn new(max_attempts: usize, window_seconds: i64, max_keys: usize) -> Self {
        Self {
            inner: Mutex::new(std::collections::HashMap::new()),
            max_attempts,
            window_seconds,
            max_keys,
        }
    }

    fn retain_recent(v: &mut Vec<i64>, cutoff: i64) {
        v.retain(|ts| *ts >= cutoff);
    }

    pub(crate) async fn is_limited(&self, key: &str, now: i64) -> bool {
        let mut m = self.inner.lock().await;
        let cutoff = now - self.window_seconds;
        let v = m.entry(key.to_string()).or_default();
        Self::retain_recent(v, cutoff);
        v.len() >= self.max_attempts
    }

    pub(crate) async fn record_failure(&self, key: &str, now: i64) {
        let mut m = self.inner.lock().await;
        let cutoff = now - self.window_seconds;
        {
            let v = m.entry(key.to_string()).or_default();
            Self::retain_recent(v, cutoff);
            v.push(now);
        }

        if m.len() > self.max_keys
            && let Some(oldest_key) = m
                .iter()
                .min_by_key(|(_, attempts)| attempts.last().copied().unwrap_or(i64::MIN))
                .map(|(k, _)| k.clone())
        {
            m.remove(&oldest_key);
        }
    }

    pub(crate) async fn clear(&self, key: &str) {
        let mut m = self.inner.lock().await;
        m.remove(key);
    }
}

impl AppState {
    pub(crate) async fn new(
        app_db: &Path,
        names_db: &Path,
        cookie_secure: bool,
        public_host: Option<String>,
        trust_proxy_headers: bool,
        proxy_header_secret: Option<String>,
    ) -> Result<Self> {
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
            login_limiter: Arc::new(LoginLimiter::new(20, 10 * 60, 5_000)),
            public_host: public_host
                .map(|h| h.trim().to_lowercase())
                .filter(|h| !h.is_empty()),
            trust_proxy_headers,
            proxy_header_secret: proxy_header_secret
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
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

#[cfg(test)]
mod tests {
    use super::LoginLimiter;

    #[tokio::test]
    async fn login_limiter_eviction_is_bounded() {
        let limiter = LoginLimiter::new(1, 60, 2);

        assert!(!limiter.is_limited("alice", 10).await);
        limiter.record_failure("alice", 10).await;
        assert!(!limiter.is_limited("bob", 11).await);
        limiter.record_failure("bob", 11).await;
        assert!(!limiter.is_limited("carol", 12).await);
        limiter.record_failure("carol", 12).await;

        // `alice` was evicted as the oldest key, so this behaves like a first attempt.
        assert!(!limiter.is_limited("alice", 13).await);
    }

    #[tokio::test]
    async fn login_limiter_clear_removes_failures() {
        let limiter = LoginLimiter::new(2, 60, 10);
        limiter.record_failure("alice", 10).await;
        limiter.record_failure("alice", 11).await;
        assert!(limiter.is_limited("alice", 12).await);
        limiter.clear("alice").await;
        assert!(!limiter.is_limited("alice", 13).await);
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
