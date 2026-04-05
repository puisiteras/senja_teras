use crate::libcwarper::utils::warp_ret;

pub fn set_hostname(name: &str) -> anyhow::Result<()> {
    warp_ret(
        unsafe { libc::sethostname(name.as_ptr() as _, name.len()) },
        "set hostname",
    )?;
    Ok(())
}

pub fn set_no_new_privs() -> anyhow::Result<()> {
    warp_ret(
        unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) },
        "set no new privs",
    )
}
