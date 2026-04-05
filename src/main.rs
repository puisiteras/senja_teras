use std::{path::PathBuf, str::FromStr};

use anyhow::Context;

use crate::libcwarper::{
    mountbuilder::MountBuilder,
    newns::NewNamespace,
    restrictor::{set_hostname, set_no_new_privs},
    utils::warp_ret,
};

mod libcwarper;

fn handler() -> anyhow::Result<()> {
    let newns = NewNamespace::new(
        libc::CLONE_NEWNS
            | libc::CLONE_NEWPID
            | libc::CLONE_NEWUTS
            | libc::CLONE_NEWIPC
            | libc::CLONE_NEWUSER,
    );

    newns.run(|| {
        println!("child handler called");
        set_hostname("senja_teras")?;

        MountBuilder::new(&PathBuf::from_str("/tmp/debugging_teras_rootfs")?)
            .context("Fail at MountBuilder::new")?
            .make_root_private()
            .context("Fail at MountBuilder::make_root_private")?
            .bind_mount_rootfs()
            .context("Fail at MountBuilder::bind_mount_fs")?
            .mount_staging_host_dev()
            .context("Fail at MountBuilder::mount_staging_host_dev")?
            .mount_before_pivot_root(
                Some(c"/nix/store"),
                c"/nix/store",
                None,
                libc::MS_BIND,
                None,
            )
            .context("Fail at MountBuilder::mount_before_pivot_root")?
            .pivot_root()
            .context("Fail at MountBuilder::pivot_root")?
            .create_minimal_dev()
            .context("Fail at MountBuilder::create_minimal_dev")?
            .mount_restricted_proc()
            .context("Fail at MountBuilder::mount_restricted_proc")?
            .link_proc_to_dev()
            .context("Fail at MountBuilder::link_proc_to_dev")?
            .unmount_staging_dev()
            .context("Fail at MountBuilder::unmount_staging_dev")?
            .deatach_old_root()
            .context("Fail at MountBuilder::deatach_old_root")?;

        set_no_new_privs()?;

        let bash_path = c"/bin/bash";
        let arg0 = c"bash";
        let argv = [arg0.as_ptr(), core::ptr::null()];
        let env = [c"PATH=/bin:/sbin".as_ptr(), core::ptr::null()];

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
