use anyhow::Result;

pub(super) fn ignore_exists(result: std::result::Result<(), rtnetlink::Error>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(rtnetlink::Error::NetlinkError(err))
            if err.code.map(std::num::NonZero::get) == Some(-libc::EEXIST) =>
        {
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

pub(super) fn ignore_notfound(result: std::result::Result<(), rtnetlink::Error>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(rtnetlink::Error::NetlinkError(err))
            if err.code.map(std::num::NonZero::get) == Some(-libc::ENOENT) =>
        {
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}
