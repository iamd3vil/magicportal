use std::collections::HashMap;

use config::{File, FileFormat};
use miette::{Context, IntoDiagnostic, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub enum Mode {
    Agent,
    Forwarder,
}

#[derive(Debug, Deserialize)]
pub struct Cfg {
    pub mode: Mode,
    pub nats: CfgNats,
    pub agent: CfgAgent,
    pub multicast_groups: Vec<CfgMulticastGroup>,

    #[serde(default = "default_packet_size")]
    pub max_packet_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct CfgNats {
    pub nats_url: Vec<String>,
    pub auth_enabled: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    pub tls: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfgAgent {
    pub send_as_unicast: bool,
    pub unicast_addrs: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfgMulticastGroup {
    pub multicast_addr: String,
    pub interface: String,
}

pub async fn parse_config(cfg_path: &str) -> Result<Cfg> {
    let settings = config::Config::builder()
        .add_source(File::new(cfg_path, FileFormat::Json))
        .build()
        .into_diagnostic()
        .wrap_err("parsing config failed")?;

    let cfg: Cfg = settings
        .try_deserialize()
        .into_diagnostic()
        .wrap_err("deserializing config failed")?;
    Ok(cfg)
}

fn default_packet_size() -> usize {
    1024
}
