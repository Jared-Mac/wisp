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
    #[arg(long, env = "WISP_PUBLIC_URL")]
    public_url: Option<String>,
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
    /// Permit profile-name development login. Defaults on only for loopback binds.
    #[arg(long, env = "WISP_ALLOW_DEV_SESSIONS")]
    allow_dev_sessions: Option<bool>,
    /// One-time administrator enrollment secret. Keep this outside the repository.
    #[arg(long, env = "WISP_BOOTSTRAP_TOKEN")]
    bootstrap_token: Option<String>,
    /// Refuse plaintext chat writes. Enable only after the coordinated cutover.
    #[arg(long, env = "WISP_REQUIRE_CHAT_E2EE", default_value_t = false)]
    require_chat_e2ee: bool,
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
    if args.require_chat_e2ee {
        anyhow::ensure!(
            args.allow_dev_sessions == Some(false),
            "Private hosting requires WISP_ALLOW_DEV_SESSIONS=false explicitly"
        );
        anyhow::ensure!(
            args.livekit_url.starts_with("wss://"),
            "Private hosting requires a TLS LiveKit URL (wss://)"
        );
        anyhow::ensure!(
            args.livekit_api_key != "devkey"
                && args.livekit_api_secret.len() >= 32
                && args.livekit_api_secret != "wisp-local-development-secret-32"
                && !args.livekit_api_secret.starts_with("replace-"),
            "Private hosting requires fresh LiveKit service credentials"
        );
        anyhow::ensure!(
            std::env::var_os("WISP_E2EE_KEY").is_none(),
            "Client media encryption keys must never be installed on the server"
        );
        anyhow::ensure!(
            args.public_url
                .as_deref()
                .is_some_and(|url| url.starts_with("https://")),
            "Private hosting requires WISP_PUBLIC_URL=https://..."
        );
    }
    let config = AppConfig {
        database_url: args.database_url.map_or_else(default_database_url, Ok)?,
        public_url: args
            .public_url
            .map(|url| url.trim_end_matches('/').to_owned()),
        livekit_url: args.livekit_url,
        livekit_api_key: args.livekit_api_key,
        livekit_api_secret: args.livekit_api_secret,
        knock_ttl: std::time::Duration::from_secs(args.knock_ttl_seconds.max(1)),
        allow_dev_sessions: args
            .allow_dev_sessions
            .unwrap_or_else(|| args.addr.ip().is_loopback()),
        bootstrap_token: args.bootstrap_token,
        require_chat_e2ee: args.require_chat_e2ee,
    };
    let state = AppState::new(config).await?;
    let maintenance = tokio::spawn(state.clone().maintain_attachments());
    let listener = tokio::net::TcpListener::bind(args.addr).await?;
    info!(address = %args.addr, "wisp-server listening");
    axum::serve(listener, wisp_server::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    maintenance.abort();
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
