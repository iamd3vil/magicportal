use miette::Result;
use tracing_subscriber::EnvFilter;

mod agent;
mod cfg;
mod forwarder;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing fmt subscriber with level from env.
    let filter = EnvFilter::from_default_env();
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Parse config.
    let cfg = cfg::parse_config("config.json").await?;

    match cfg.mode {
        cfg::Mode::Agent => {
            println!("Agent mode");
            agent::start_agent(cfg).await?;
        }
        cfg::Mode::Forwarder => {
            forwarder::start_forwarder(cfg).await?;
        }
    }

    Ok(())
}
