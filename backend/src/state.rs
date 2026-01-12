use std::path::Path;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Row, Sqlite};

#[derive(Debug, Clone, Copy)]
pub(crate) struct NamesWindow {
    pub(crate) run_id: i64,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: Pool<Sqlite>,
    pub(crate) names_db: Pool<Sqlite>,
    pub(crate) names_window: NamesWindow,
    pub(crate) country_total_births: CountryTotals,
    pub(crate) cookie_secure: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CountryTotals {
    pub(crate) us_total: i64,
    pub(crate) fr_total: i64,
    pub(crate) cl_total: i64,
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

        let names_window = load_names_window(&names_db).await?;
        let country_total_births = load_country_totals(&names_db, names_window.run_id).await?;

        Ok(Self {
            db,
            names_db,
            names_window,
            country_total_births,
            cookie_secure,
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

async fn load_names_window(names_db: &Pool<Sqlite>) -> Result<NamesWindow> {
    let row = sqlx::query("SELECT id FROM import_run ORDER BY id DESC LIMIT 1")
        .fetch_one(names_db)
        .await
        .context("load names window")?;

    Ok(NamesWindow {
        run_id: row.try_get("id")?,
    })
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
