use std::io;

pub fn warp_ret(ret: i32, context: &str) -> anyhow::Result<()> {
    if ret != 0 {
        Err(anyhow::anyhow!(
            "libc call error[ret={}, context={}]: {:?}",
            ret,
            context,
            io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}
