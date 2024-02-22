use gumdrop::Options;
use miette::Result;
use tracing_subscriber::EnvFilter;

mod agent;
mod cfg;
mod forwarder;

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
