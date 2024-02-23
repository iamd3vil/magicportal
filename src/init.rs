use crate::cfg::CfgNats;
use async_nats::Client;
use miette::{miette, Context, IntoDiagnostic, Result};

pub async fn init_nats(cfg: &CfgNats) -> Result<Client> {
    let mut options = async_nats::ConnectOptions::new().require_tls(cfg.tls);
    if cfg.auth_enabled {
        if cfg.username.is_none() {
            return Err(miette!("username can't be empty"));
        }
        if cfg.password.is_none() {
            return Err(miette!("password can't be empty"));
        }
        let username = cfg.username.as_ref().unwrap();
        let password = cfg.password.as_ref().unwrap();
        options = options.user_and_password(username.to_string(), password.to_string());
    }

    options
        .connect(cfg.nats_url.join(","))
        .await
        .into_diagnostic()
        .wrap_err("connecting to NATS failed")
}
