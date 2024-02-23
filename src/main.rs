use gumdrop::Options;
use miette::{IntoDiagnostic, Result};
use tokio::{signal, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod agent;
mod cfg;
mod forwarder;
mod init;

#[derive(Debug, Options)]
struct Args {
    #[options(help = "print help message")]
    help: bool,

    #[options(help = "configuration path")]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing fmt subscriber with level from env.
    let filter = EnvFilter::from_default_env();
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let opts = Args::parse_args_default_or_exit();

    // Parse config.
    let cfg = cfg::parse_config(&opts.config.unwrap_or("config.json".to_string())).await?;

    let cancel_token = CancellationToken::new();

    let handle: JoinHandle<_>;
    match cfg.mode {
        cfg::Mode::Agent => {
            let cancel_token = cancel_token.clone();
            handle = tokio::spawn(async move { agent::start_agent(cfg, cancel_token).await });
        }
        cfg::Mode::Forwarder => {
            let cancel_token = cancel_token.clone();
            handle =
                tokio::spawn(async move { forwarder::start_forwarder(cfg, cancel_token).await });
        }
    }

    // Spawn to wait for ctrl_c and cancel.
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        info!("received exit signal...");
        cancel_token.cancel();
    });

    let _ = handle.await.into_diagnostic()?;

    info!("exiting...");

    Ok(())
}
