use std::path::Path;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: Pool<Sqlite>,
    pub(crate) cookie_secure: bool,
}

impl AppState {
    pub(crate) async fn new(app_db: &Path, cookie_secure: bool) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(app_db)
            .create_if_missing(true)
            .foreign_keys(true);

        let db = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
            .with_context(|| format!("open app db {app_db:?}"))?;

        Ok(Self { db, cookie_secure })
    }

    pub(crate) async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.db)
            .await
            .context("run migrations")?;
        Ok(())
    }
}
