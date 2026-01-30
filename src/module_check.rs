use anyhow::{Context, Result};
use rtnetlink::LinkWireguard;

use crate::netlink_util::{get_link_by_name, netlink_err_code};

const CHECK_IFACE: &str = "wg-check";

pub async fn ensure_wireguard_support() -> Result<()> {
    let (connection, handle, _) = rtnetlink::new_connection().context("opening netlink")?;
    tokio::spawn(connection);

    if let Some(link) = get_link_by_name(&handle, CHECK_IFACE).await? {
        let _ = handle.link().del(link.header.index).execute().await;
    }

    let add = handle
        .link()
        .add(LinkWireguard::new(CHECK_IFACE).build())
        .execute()
        .await;

    if let Err(err) = add {
        if let Some(code) = netlink_err_code(&err) {
            if code == -libc::EPERM {
                anyhow::bail!(
                    "wireguard interface creation failed: missing CAP_NET_ADMIN or insufficient privileges"
                );
            }
            if code == -libc::EOPNOTSUPP || code == -libc::ENODEV {
                anyhow::bail!("wireguard interface creation failed: kernel module not available");
            }
        }
        return Err(err).context("creating wireguard interface for module check");
    }

    if let Some(link) = get_link_by_name(&handle, CHECK_IFACE).await? {
        let _ = handle.link().del(link.header.index).execute().await;
    }

    Ok(())
}
