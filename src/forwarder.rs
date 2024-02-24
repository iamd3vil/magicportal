use miette::{miette, Context, IntoDiagnostic, Result};
use netdev;
use std::net::{Ipv4Addr, SocketAddr};
// use network_interface::{NetworkInterface, NetworkInterfaceConfig};
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
        handle.await.into_diagnostic().wrap_err("task failed")?;
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

    let interface_ip = get_interface_ip(&grp.interface)?;

    // Start a multicast UDP listener.
    let socket = UdpSocket::bind(grp.multicast_addr.clone())
        .await
        .into_diagnostic()
        .wrap_err("binding to multicast address failed")?;

    debug!(multicast_addr = %grp.multicast_addr, "Bound to multicast address");

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

fn get_interface_ip(interface_name: &str) -> Result<std::net::Ipv4Addr> {
    // let interfaces = NetworkInterface::show()
    //     .into_diagnostic()
    //     .wrap_err("getting interface list")?;

    // for inf in interfaces.iter() {
    //     if inf.name == interface_name {
    //         if inf.addr.is_empty() {
    //             return Err(miette!("error getting interface ip"));
    //         }

    //         // Return the first IPv4 address.
    //         for addr in inf.addr.iter() {
    //             if let IpAddr::V4(ip) = addr.ip() {
    //                 return Ok(ip);
    //             }
    //         }
    //     }
    // }
    // Err(miette!("error getting interface ip"))
    let interfaces = netdev::get_interfaces();
    for interface in interfaces {
        if interface.name == interface_name {
            return Ok(interface.ipv4[0].addr);
        }
    }

    Err(miette!("error getting interface ip"))
}
