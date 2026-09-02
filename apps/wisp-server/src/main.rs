use anyhow::Context;
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf};
use tracing::info;
use tracing_subscriber::EnvFilter;
use wisp_server::{AppConfig, AppState};

#[derive(Debug, Parser)]
#[command(about = "Wisp's loopback coordination server")]
struct Args {
    #[arg(long, env = "WISP_SERVER_ADDR", default_value = "127.0.0.1:8787")]
    addr: SocketAddr,
    #[arg(long, env = "WISP_DATABASE_URL")]
    database_url: Option<String>,
    #[arg(long, env = "WISP_LIVEKIT_URL", default_value = "ws://127.0.0.1:7880")]
    livekit_url: String,
    #[arg(long, env = "WISP_LIVEKIT_API_KEY", default_value = "devkey")]
    livekit_api_key: String,
    #[arg(
        long,
        env = "WISP_LIVEKIT_API_SECRET",
        default_value = "wisp-local-development-secret-32"
    )]
    livekit_api_secret: String,
    #[arg(long, env = "WISP_KNOCK_TTL_SECONDS", default_value_t = 30)]
    knock_ttl_seconds: u64,
}

fn default_database_url() -> anyhow::Result<String> {
    let data_home = std::env::var_os("XDG_DATA_HOME").map_or_else(
        || {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".local/share"))
        },
        |path| Some(PathBuf::from(path)),
    );
    let path = data_home
        .context("HOME or XDG_DATA_HOME must be set")?
        .join("wisp/server/wisp.sqlite3");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create Wisp data directory")?;
    }
    Ok(format!("sqlite://{}", path.display()))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wisp_server=info,tower_http=info".into()),
        )
        .init();

    let args = Args::parse();
    let config = AppConfig {
        database_url: args.database_url.map_or_else(default_database_url, Ok)?,
        livekit_url: args.livekit_url,
        livekit_api_key: args.livekit_api_key,
        livekit_api_secret: args.livekit_api_secret,
        knock_ttl: std::time::Duration::from_secs(args.knock_ttl_seconds.max(1)),
    };
    let state = AppState::new(config).await?;
    let listener = tokio::net::TcpListener::bind(args.addr).await?;
    info!(address = %args.addr, "wisp-server listening");
    axum::serve(listener, wisp_server::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { () = ctrl_c => {}, () = terminate => {} }
}
