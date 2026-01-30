use crate::config::ResolvedConfig;
use crate::wg_iface::peers::peer_allowed_ips;
use crate::wg_iface::util::{ignore_exists, ignore_notfound};
use anyhow::{Context, Result};
use ipnet::IpNet;
use rtnetlink::RouteMessageBuilder;

pub(super) async fn configure_routes(
    handle: &rtnetlink::Handle,
    link_index: u32,
    config: &ResolvedConfig,
) -> Result<()> {
    for peer in &config.peers {
        let peer_dir = config.paths.peers.join(&peer.id);
        for allowed in peer_allowed_ips(&peer_dir)? {
            let ipnet: IpNet = allowed
                .parse()
                .with_context(|| format!("parsing allowed ip {allowed}"))?;
            let res = match ipnet {
                IpNet::V4(v4) => {
                    handle
                        .route()
                        .add(
                            RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                                .output_interface(link_index)
                                .destination_prefix(v4.addr(), v4.prefix_len())
                                .build(),
                        )
                        .execute()
                        .await
                }
                IpNet::V6(v6) => {
                    handle
                        .route()
                        .add(
                            RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                                .output_interface(link_index)
                                .destination_prefix(v6.addr(), v6.prefix_len())
                                .build(),
                        )
                        .execute()
                        .await
                }
            };
            ignore_exists(res).with_context(|| format!("adding route {allowed}"))?;
        }
    }
    Ok(())
}

pub(super) async fn delete_routes(
    handle: &rtnetlink::Handle,
    link_index: u32,
    config: &ResolvedConfig,
) -> Result<()> {
    for peer in &config.peers {
        let peer_dir = config.paths.peers.join(&peer.id);
        for allowed in peer_allowed_ips(&peer_dir)? {
            let ipnet: IpNet = allowed
                .parse()
                .with_context(|| format!("parsing allowed ip {allowed}"))?;
            let message = match ipnet {
                IpNet::V4(v4) => RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                    .output_interface(link_index)
                    .destination_prefix(v4.addr(), v4.prefix_len())
                    .build(),
                IpNet::V6(v6) => RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                    .output_interface(link_index)
                    .destination_prefix(v6.addr(), v6.prefix_len())
                    .build(),
            };
            let res = handle.route().del(message).execute().await;
            ignore_notfound(res).with_context(|| format!("deleting route {allowed}"))?;
        }
    }
    Ok(())
}
