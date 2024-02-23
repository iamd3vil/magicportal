use std::net::{Ipv4Addr, SocketAddr};

use miette::{miette, Context, IntoDiagnostic, Result};
use pnet::datalink;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::{
    cfg::{Cfg, CfgMulticastGroup},
    init,
};

pub async fn start_forwarder(cfg: Cfg, cancel_token: CancellationToken) -> Result<()> {
    info!("Starting forwarder...");

    // Start a NATS client.
    let nc = init::init_nats(&cfg.nats).await?;

    debug!("Connected to NATS");

    let grps = cfg.multicast_groups.clone();

    // Start a task for each multicast group.
    let mut handles = Vec::new();
    for grp in grps {
        let nc = nc.clone();
        let cancel_token = cancel_token.clone();
        debug!(multicast_addr = %grp.multicast_addr, "Starting task for multicast group");
        let handle = tokio::spawn(async move {
            let res = process_multicast(&grp, &nc, cfg.max_packet_size, cancel_token).await;
            if let Err(e) = res {
                error!(error = %e, "Task failed");
            }
        });
        handles.push(handle);
    }

    debug!("All tasks started");

    // Wait for all tasks to finish.
    for handle in handles {
        let _ = handle.await.into_diagnostic().wrap_err("task failed")?;
    }

    Ok(())
}

pub async fn process_multicast(
    grp: &CfgMulticastGroup,
    nc: &async_nats::Client,
    max_packet_size: usize,
    cancel_token: CancellationToken,
) -> Result<()> {
    // Convert multicast addr to SocketAddr.
    let addr = grp
        .multicast_addr
        .parse::<SocketAddr>()
        .into_diagnostic()
        .wrap_err("parsing multicast address failed")?;

    let addr_ip: Ipv4Addr = addr
        .ip()
        .to_string()
        .parse()
        .into_diagnostic()
        .wrap_err("parsing multicast address failed")?;

    let interface_ip = get_interface_ip(&grp.interface);

    if interface_ip.is_none() {
        return Err(miette!("invalid interface"));
    }

    let interface_ip = interface_ip.unwrap();

    // Start a multicast UDP listener.
    let socket = UdpSocket::bind(grp.multicast_addr.clone())
        .await
        .into_diagnostic()
        .wrap_err("binding to multicast address failed")?;

    debug!(multicast_addr = %grp.multicast_addr, "Bound to multicast address");

    // Join the multicast group.
    socket
        .join_multicast_v4(addr_ip, interface_ip)
        .into_diagnostic()
        .wrap_err("joining multicast group failed")?;

    debug!(multicast_addr = %grp.multicast_addr, "Joined multicast group");

    // Receive packets and forward them to NATS.
    let mut buf = vec![0u8; max_packet_size];
    loop {
        tokio::select! {
            packet = socket.recv_from(&mut buf) => {
                if let Err(err) = packet {
                    return Err(err).into_diagnostic().wrap_err("receving from socket");
                }
                let (len, _) = packet.unwrap();
                debug!(length = len, "Received packet");

                let payload = buf[..len].to_vec();

                nc.publish(grp.multicast_addr.clone(), payload.into())
                    .await
                    .into_diagnostic()
                    .wrap_err("publishing to NATS failed")?;
            }
            _ = cancel_token.cancelled() => {
                debug!(grp = grp.multicast_addr, "exiting loop");
                return Ok(())
            }
        }
    }
}

fn get_interface_ip(interface_name: &str) -> Option<std::net::Ipv4Addr> {
    for interface in datalink::interfaces() {
        if interface.name == interface_name {
            return interface.ips.iter().find_map(|ip| {
                match ip.ip() {
                    std::net::IpAddr::V4(ipv4) => Some(ipv4),
                    _ => None, // Ignore IPv6 for now
                }
            });
        }
    }
    None // Interface not found
}
