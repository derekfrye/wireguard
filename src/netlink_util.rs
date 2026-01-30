use anyhow::{Context, Result};
use futures::TryStreamExt;
use netlink_packet_route::link::LinkMessage;
use rtnetlink::Handle;

pub async fn get_link_by_name(handle: &Handle, name: &str) -> Result<Option<LinkMessage>> {
    match handle
        .link()
        .get()
        .match_name(name.to_string())
        .execute()
        .try_next()
        .await
    {
        Ok(link) => Ok(link),
        Err(err) => {
            if let Some(code) = netlink_err_code(&err)
                && (code == -libc::ENODEV || code == -libc::ENOENT) {
                    return Ok(None);
                }
            Err(err).with_context(|| format!("getting link {name}"))
        }
    }
}

pub fn netlink_err_code(err: &rtnetlink::Error) -> Option<i32> {
    match err {
        rtnetlink::Error::NetlinkError(netlink) => netlink.code.map(|c| c.get()),
        _ => None,
    }
}
