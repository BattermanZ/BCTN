mod bootstrap;
mod display;
mod routes;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use axum::body::Body;
use axum::http::HeaderName;
use axum::http::HeaderValue;
use axum::http::Request;
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use clap::Parser;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::bootstrap::bootstrap_admin;
use crate::routes::router;
use crate::state::AppState;

#[derive(Debug, Parser)]
#[command(name = "bctn", version)]
struct Args {
    #[arg(long, env = "BCTN_APP_DB", default_value = "data/bctn_app.sqlite")]
    app_db: PathBuf,

    #[arg(long, env = "BCTN_NAMES_DB", default_value = "data/bctn.sqlite")]
    names_db: PathBuf,

    /// Directory of static assets to serve (PWA build output).
    #[arg(long, env = "BCTN_STATIC_DIR", default_value = "web/build")]
    static_dir: PathBuf,

    /// Mark session cookies as Secure (required behind HTTPS).
    #[arg(long, env = "BCTN_COOKIE_SECURE", default_value_t = true)]
    cookie_secure: bool,

    /// Optional public host allowlist (e.g. `bctn.example.com`); rejects other Host values.
    #[arg(long, env = "BCTN_PUBLIC_HOST")]
    public_host: Option<String>,

    /// Trust reverse-proxy forwarded headers (`X-Forwarded-Host`) for host/origin checks.
    /// Keep disabled unless traffic always comes through a trusted proxy.
    #[arg(long, env = "BCTN_TRUST_PROXY_HEADERS", default_value_t = false)]
    trust_proxy_headers: bool,

    /// Shared secret required to trust forwarded host headers.
    /// If set, requests must include matching `X-BCTN-Proxy-Secret`.
    #[arg(long, env = "BCTN_PROXY_HEADER_SECRET")]
    proxy_header_secret: Option<String>,

    /// Bootstrap the first admin user if the DB has no users (exits after running).
    #[arg(long, env = "BCTN_BOOTSTRAP_ADMIN_USERNAME")]
    bootstrap_admin_username: Option<String>,

    /// Password for `--bootstrap-admin-username` (min 8 chars).
    #[arg(long, env = "BCTN_BOOTSTRAP_ADMIN_PASSWORD")]
    bootstrap_admin_password: Option<String>,
}

async fn no_store_html(req: Request<Body>, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let is_html = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("text/html"));
    if is_html {
        resp.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    resp
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

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

    let state = AppState::new(
        &args.app_db,
        &args.names_db,
        args.cookie_secure,
        args.public_host,
        args.trust_proxy_headers,
        args.proxy_header_secret,
    )
    .await?;
    state.migrate().await?;

    if args.bootstrap_admin_username.is_some() || args.bootstrap_admin_password.is_some() {
        let Some(username) = args.bootstrap_admin_username else {
            bail!("missing --bootstrap-admin-username");
        };
        let Some(password) = args.bootstrap_admin_password else {
            bail!("missing --bootstrap-admin-password");
        };
        bootstrap_admin(&state.db, &username, &password).await?;
        tracing::info!("bootstrap_admin_done");
        return Ok(());
    }

    let static_service = ServeDir::new(&args.static_dir).append_index_html_on_directories(true);
    let mut app = router(state)
        .nest_service("/", static_service)
        .layer(axum::middleware::from_fn(no_store_html))
        .layer(TraceLayer::new_for_http())
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
            ),
        ));

    if args.cookie_secure {
        app = app.layer(SetResponseHeaderLayer::overriding(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ));
    }

    let bind: SocketAddr = "0.0.0.0:2286".parse().expect("valid bind addr");
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("bind {bind}"))?;
    tracing::info!(bind=%bind, static_dir=%args.static_dir.display(), "backend_start");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("serve")?;
    Ok(())
}
