use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use async_nats::Client;
use futures::StreamExt;
use miette::{miette, Context, IntoDiagnostic, Result};
use tokio::{net::UdpSocket, select};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::{
    cfg::{Cfg, CfgMulticastGroup},
    init,
};

pub async fn start_agent(cfg: Cfg, cancel_token: CancellationToken) -> Result<()> {
    info!("Starting agent...");

    // Start a NATS client.
    let nc = init::init_nats(&cfg.nats).await?;

    let grps = cfg.multicast_groups.clone();
    let unicast_addrs = Arc::new(cfg.agent.unicast_addrs);
    // Start a task for each multicast group.
    let mut handles = Vec::new();
    for grp in grps {
        let nc = nc.clone();
        let unicast_addrs = unicast_addrs.clone();
        let cancel_token = cancel_token.clone();

        // Spawn a tokio task for each multicast group.
        let handle = tokio::spawn(async move {
            let res = subscribe_and_process(
                &grp,
                nc,
                cfg.agent.send_as_unicast,
                unicast_addrs,
                cancel_token,
            )
            .await;
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
    cancel_token: CancellationToken,
) -> Result<()> {
    info!(group = grp.multicast_addr, "waiting for messages");

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
    loop {
        select! {
            Some(msg) = sub.next() => {
                debug!(
                    len = msg.payload.len(),
                    grp = grp.multicast_addr,
                    "Received message"
                );

                socket
                    .send(&msg.payload)
                    .await
                    .into_diagnostic()
                    .wrap_err("sending message to socket")?;
            }
            _ = cancel_token.cancelled() => {
                debug!(grp = grp.multicast_addr, "exiting loop");
                return Ok(())
            }
        }
    }
}
