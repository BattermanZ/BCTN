mod routes;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::routes::router;
use crate::state::AppState;

#[derive(Debug, Parser)]
#[command(name = "bctn", version)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: SocketAddr,

    #[arg(long, default_value = "data/bctn_app.sqlite")]
    app_db: PathBuf,

    /// Directory of static assets to serve (PWA build output).
    #[arg(long, default_value = "web/public")]
    static_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let Some(parent) = args.app_db.parent() else {
        bail!("app_db must have a parent directory");
    };
    tokio::fs::create_dir_all(parent)
        .await
        .with_context(|| format!("create dir {parent:?}"))?;

    let state = AppState::new(&args.app_db).await?;
    state.migrate().await?;

    let static_service = ServeDir::new(&args.static_dir).append_index_html_on_directories(true);
    let app = router(state)
        .nest_service("/", static_service)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(args.bind)
        .await
        .with_context(|| format!("bind {}", args.bind))?;
    tracing::info!(bind=%args.bind, static_dir=%args.static_dir.display(), "backend_start");
    axum::serve(listener, app).await.context("serve")?;
    Ok(())
}
