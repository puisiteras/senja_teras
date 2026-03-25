use crate::libcwarper::{newns::NewNamespace, utils::warp_ret};

mod libcwarper;

fn handler() -> anyhow::Result<()> {
    let newns = NewNamespace::new(
        libc::CLONE_NEWNS | libc::CLONE_NEWPID | libc::CLONE_NEWUTS | libc::CLONE_NEWIPC,
    );

    newns.run(|| {
        println!("child handler called");

        let bash_path = c"/bin/bash";
        let arg0 = c"bash";
        let argv = [arg0.as_ptr(), std::ptr::null()];
        let env = [std::ptr::null()];

        warp_ret(
            unsafe { libc::execve(bash_path.as_ptr(), argv.as_ptr(), env.as_ptr()) },
            "execute bash",
        )?;

        Ok(())
    })?;

    Ok(())
}

fn main() {
    match handler() {
        Ok(()) => {}
        Err(err) => {
            eprintln!("Handler Error:\n{:?}", err);
        }
    }
}
