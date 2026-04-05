use std::env::current_dir;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn warp_ret(ret: i32, context: &str) -> anyhow::Result<()> {
    if ret != 0 {
        Err(anyhow::anyhow!(
            "libc call error[ret={}, context={}]: {:?}",
            ret,
            context,
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

pub fn warp_io_call<T>(result: std::io::Result<T>, context: &str) -> anyhow::Result<T> {
    if let Err(err) = result {
        Err(anyhow::anyhow!(
            "io call error[context={}]: {:?}",
            context,
            err
        ))
    } else {
        Ok(result.unwrap())
    }
}

static CWD_CACHE: OnceLock<PathBuf> = OnceLock::new();
pub fn get_cwd() -> &'static Path {
    CWD_CACHE
        .get_or_init(|| current_dir().expect("panic: fail read current dir"))
        .as_path()
}
