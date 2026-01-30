use crate::netlink_util::get_link_by_name;
use crate::wg_iface::WG_IFACE;
use anyhow::{Context, Result};
use rtnetlink::{LinkUnspec, LinkWireguard};

pub(super) async fn ensure_wireguard_link(handle: &rtnetlink::Handle) -> Result<u32> {
    if let Some(link) = get_link_by_name(handle, WG_IFACE).await? {
        return Ok(link.header.index);
    }

    handle
        .link()
        .add(LinkWireguard::new(WG_IFACE).build())
        .execute()
        .await
        .context("creating wireguard link")?;

    let link = get_link_by_name(handle, WG_IFACE)
        .await?
        .context("wireguard link missing after creation")?;

    Ok(link.header.index)
}

pub(super) async fn set_link_up(handle: &rtnetlink::Handle, link_index: u32) -> Result<()> {
    handle
        .link()
        .set(LinkUnspec::new_with_index(link_index).up().build())
        .execute()
        .await
        .context("setting wg link up")
}
