use std::{collections::HashMap, path::PathBuf};

use config::{File, FileFormat};
use miette::{miette, Context, IntoDiagnostic, Result};
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
    // Check file extension.
    let path = PathBuf::from(cfg_path);
    if let None = path.extension() {
        return Err(miette!(
            "config file extension should be either .json or .toml"
        ));
    }
    let ext = path.extension().unwrap();
    let formatter: FileFormat;
    match ext.to_str() {
        Some("json") => formatter = FileFormat::Json,
        Some("toml") => formatter = FileFormat::Toml,
        _ => {
            return Err(miette!(
                "config file extension should be either .json or .toml"
            ));
        }
    }

    let settings = config::Config::builder()
        .add_source(File::new(cfg_path, formatter))
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
