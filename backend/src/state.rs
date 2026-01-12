use std::path::Path;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: Pool<Sqlite>,
}

impl AppState {
    pub(crate) async fn new(app_db: &Path) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(app_db)
            .create_if_missing(true)
            .foreign_keys(true);

        let db = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
            .with_context(|| format!("open app db {app_db:?}"))?;

        Ok(Self { db })
    }

    pub(crate) async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.db)
            .await
            .context("run migrations")?;
        Ok(())
    }
}
