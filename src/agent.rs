use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use async_nats::Client;
use futures::StreamExt;
use miette::{miette, Context, IntoDiagnostic, Result};
use tokio::net::UdpSocket;
use tracing::{error, info};

use crate::cfg::{Cfg, CfgMulticastGroup};

pub async fn start_agent(cfg: Cfg) -> Result<()> {
    info!("Starting agent with config: {:?}", cfg);

    // Start a NATS client.
    let nc = async_nats::connect(&cfg.nats.nats_url.join(","))
        .await
        .into_diagnostic()
        .wrap_err("connecting to NATS failed")?;

    let grps = cfg.multicast_groups.clone();
    let unicast_addrs = Arc::new(cfg.agent.unicast_addrs);
    // Start a task for each multicast group.
    let mut handles = Vec::new();
    for grp in grps {
        let nc = nc.clone();
        let unicast_addrs = unicast_addrs.clone();

        // Spawn a tokio task for each multicast group.
        let handle = tokio::spawn(async move {
            let res =
                subscribe_and_process(&grp, nc, cfg.agent.send_as_unicast, unicast_addrs).await;
            if res.is_err() {
                // Log the error.
                error!(error = %res.err().unwrap(), "Task failed");
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to finish.
    for handle in handles {
        let _ = handle.await.into_diagnostic().wrap_err("task failed")?;
    }

    Ok(())
}

async fn subscribe_and_process(
    grp: &CfgMulticastGroup,
    nc: Client,
    send_as_unicast: bool,
    unicast_addrs: Arc<Option<HashMap<String, String>>>,
) -> Result<()> {
    if send_as_unicast {
        if unicast_addrs.is_none() {
            return Err(miette!(
                "unicast_addrs can't be empty if send_as_unicast is true"
            ));
        }
    }

    // Subscribe to the NATS subject.
    let mut sub = nc
        .subscribe(grp.multicast_addr.clone())
        .await
        .into_diagnostic()
        .wrap_err("subscribing to NATS failed")?;

    let unicast_addrs = Option::clone(&unicast_addrs.as_ref()).unwrap();

    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .into_diagnostic()
        .wrap_err("error binding to local addr")?;

    if send_as_unicast {
        // Check if the passed unicast_addrs has the grp.
        if !unicast_addrs.contains_key(&grp.multicast_addr) {
            return Err(miette!(
                "unicast_addrs doesn't contain the multicast group: {}",
                grp.multicast_addr
            ));
        }

        let addr = &unicast_addrs[&grp.multicast_addr];

        let addr: SocketAddr = addr
            .parse()
            .into_diagnostic()
            .wrap_err("error parsing addr")?;

        socket
            .connect(addr)
            .await
            .into_diagnostic()
            .wrap_err("connecting to unicast addr")?;
    } else {
        let addr: SocketAddr = grp
            .multicast_addr
            .parse()
            .into_diagnostic()
            .wrap_err("parsing multicast addr")?;

        socket
            .connect(addr)
            .await
            .into_diagnostic()
            .wrap_err(format!(
                "connecting to multicast group: {}",
                grp.multicast_addr
            ))?;
    }

    // Process messages.
    while let Some(msg) = sub.next().await {
        info!("Received message");

        socket
            .send(&msg.payload)
            .await
            .into_diagnostic()
            .wrap_err("sending message to socket")?;
    }

    Ok(())
}
