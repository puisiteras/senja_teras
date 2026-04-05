use core::ptr;
use std::{
    ffi::{CStr, CString},
    fs, io,
    path::Path,
    str::FromStr,
};

use path_absolutize::Absolutize;

use crate::libcwarper::utils::{get_cwd, warp_io_call, warp_ret};

pub struct MountBuilder {
    rootfs: CString,
}

impl MountBuilder {
    pub fn new(rootfs: &Path) -> anyhow::Result<Self> {
        let p = rootfs.absolutize_from(get_cwd())?;

        let metadata = warp_io_call(fs::metadata(&p), "reading metadata rootfs")?;

        if !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "rootfs is exist but not an directory",
            )
            .into());
        }
        Ok(Self {
            rootfs: CString::from_str(
                p.as_os_str()
                    .to_str()
                    .ok_or(anyhow::anyhow!("path rootfs invalid UTF-8"))?,
            )?,
        })
    }

    pub fn make_root_private(&self) -> anyhow::Result<&Self> {
        let root = c"/".as_ptr();
        warp_ret(
            unsafe {
                libc::mount(
                    ptr::null(),
                    root,
                    ptr::null(),
                    libc::MS_PRIVATE | libc::MS_REC,
                    ptr::null(),
                )
            },
            "mount make '/' private",
        )?;
        Ok(self)
    }

    pub fn bind_mount_rootfs(&self) -> anyhow::Result<&Self> {
        let target = self.rootfs.as_ptr();
        warp_ret(
            unsafe {
                libc::mount(
                    target,
                    target,
                    ptr::null(),
                    libc::MS_BIND | libc::MS_REC,
                    ptr::null(),
                )
            },
            "bind mount rootfs to itself",
        )?;

        warp_ret(
            unsafe {
                libc::mount(
                    ptr::null(),
                    target,
                    ptr::null(),
                    libc::MS_PRIVATE | libc::MS_REC,
                    ptr::null(),
                )
            },
            "make rootfs private",
        )?;
        Ok(self)
    }

    /// check first char is '/' or '\' and join to roofs path
    /// Return Rootfs path + target
    fn validate_target(&self, target: &CStr) -> anyhow::Result<CString> {
        let target_bytes = target.to_bytes_with_nul();
        let first_byte = target_bytes[0];
        if first_byte == 47 || first_byte == 92 {
            let mut cstring_data =
                Vec::with_capacity(self.rootfs.as_bytes().len() + target_bytes.len());
            cstring_data.extend_from_slice(self.rootfs.as_bytes());
            cstring_data.extend_from_slice(target_bytes);

            Ok(CString::from_vec_with_nul(cstring_data)?)
        } else {
            Err(anyhow::anyhow!(format!(
                "target must startwith / or \\ because will join to rootfs path\nInputed: {:?}",
                String::from_utf8_lossy(target_bytes)
            )))
        }
    }

    pub fn mount_before_pivot_root(
        &self,
        src: Option<&CStr>,
        target: &CStr,
        fstype: Option<&CStr>,
        flags: u64,
        data: Option<&CStr>,
    ) -> anyhow::Result<&Self> {
        let target = self.validate_target(target)?;
        fs::create_dir_all(target.to_str()?)?;
        warp_ret(
            unsafe {
                libc::mount(
                    src.map_or(fstype.map_or(ptr::null(), |s| s.as_ptr()), |s| s.as_ptr()),
                    target.as_ptr(),
                    fstype.map_or(ptr::null(), |s| s.as_ptr()),
                    flags,
                    data.map_or(ptr::null(), |s| s.as_ptr() as _),
                )
            },
            "create mount in MountBuilder",
        )?;
        Ok(self)
    }

    pub fn mount_after_pivot_root(
        &self,
        src: Option<&CStr>,
        target: &CStr,
        fstype: Option<&CStr>,
        flags: u64,
        data: Option<&CStr>,
    ) -> anyhow::Result<&Self> {
        let first_byte = target.to_bytes()[0];
        if first_byte == 47 || first_byte == 92 {
            fs::create_dir_all(target.to_str()?)?;
            warp_ret(
                unsafe {
                    libc::mount(
                        src.map_or(fstype.map_or(ptr::null(), |s| s.as_ptr()), |s| s.as_ptr()),
                        target.as_ptr(),
                        fstype.map_or(ptr::null(), |s| s.as_ptr()),
                        flags,
                        data.map_or(ptr::null(), |s| s.as_ptr() as _),
                    )
                },
                "create mount in MountBuilder",
            )?;
            Ok(self)
        } else {
            Err(anyhow::anyhow!(
                "target must startwith / or \\ because will join to rootfs path",
            ))
        }
    }

    pub fn pivot_root(&self) -> anyhow::Result<&Self> {
        let rootfs_bytes = self.rootfs.as_bytes();
        let put_old_bytes = c"/.old_root".to_bytes_with_nul();
        let mut put_old = Vec::with_capacity(rootfs_bytes.len() + put_old_bytes.len());
        put_old.extend_from_slice(rootfs_bytes);
        put_old.extend_from_slice(put_old_bytes);
        let put_old = CString::from_vec_with_nul(put_old)?;

        fs::create_dir_all(put_old.to_str()?)?;

        warp_ret(
            unsafe {
                libc::syscall(libc::SYS_pivot_root, self.rootfs.as_ptr(), put_old.as_ptr()) as _
            },
            "pivoting root",
        )?;

        warp_ret(unsafe { libc::chdir(c"/".as_ptr()) }, "chdir to /")?;

        Ok(self)
    }

    pub fn deatach_old_root(&self) -> anyhow::Result<&Self> {
        warp_ret(
            unsafe { libc::umount2(c"/.old_root".as_ptr(), libc::MNT_DETACH) },
            "deatach old root",
        )?;
        Ok(self)
    }

    pub fn mount_staging_host_dev(&self) -> anyhow::Result<&Self> {
        self.mount_before_pivot_root(
            Some(c"/dev"),
            c"/.host/dev",
            None,
            libc::MS_BIND | libc::MS_REC,
            None,
        )
    }

    pub fn unmount_staging_dev(&self) -> anyhow::Result<&Self> {
        warp_ret(
            unsafe { libc::umount2(c"/.host/dev".as_ptr(), libc::MNT_DETACH) },
            "deatach host dev",
        )?;
        Ok(self)
    }

    pub fn symlink(&self, original: &CStr, linkpath: &CStr) -> anyhow::Result<&Self> {
        let _ = fs::remove_file(linkpath.to_str()?);
        warp_ret(
            unsafe { libc::symlink(original.as_ptr(), linkpath.as_ptr()) },
            "create symlink",
        )?;
        Ok(self)
    }

    pub fn mount_dev(&self, dev_path: &CStr) -> anyhow::Result<&Self> {
        let src = CString::new(format!("/.host{}", dev_path.to_str()?))?;

        fs::File::create(dev_path.to_str()?)?;
        unsafe {
            warp_ret(
                libc::mount(
                    src.as_ptr(),
                    dev_path.as_ptr(),
                    ptr::null(),
                    libc::MS_BIND,
                    ptr::null(),
                ),
                "mount dev",
            )?
        }
        Ok(self)
    }

    pub fn create_minimal_dev(&self) -> anyhow::Result<&Self> {
        self.mount_after_pivot_root(
            None,
            c"/dev",
            Some(c"tmpfs"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            Some(c"mode=0755,size=4m"),
        )?
        .mount_dev(c"/dev/null")?
        .mount_dev(c"/dev/zero")?
        .mount_dev(c"/dev/full")?
        .mount_dev(c"/dev/random")?
        .mount_dev(c"/dev/urandom")?
        .mount_dev(c"/dev/tty")?
        .mount_after_pivot_root(
            None,
            c"/dev/pts",
            Some(c"devpts"),
            libc::MS_NOSUID | libc::MS_NOEXEC,
            Some(c"newinstance,ptmxmode=0666,mode=0620"),
        )?
        .symlink(c"/dev/pts/ptmx", c"/dev/ptmx")?
        .mount_after_pivot_root(
            None,
            c"/dev/mqueue",
            Some(c"mqueue"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            None,
        )?
        .mount_after_pivot_root(
            None,
            c"/dev/shm",
            Some(c"tmpfs"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            Some(c"mode=1777,size=16m"),
        )?
        .mount_after_pivot_root(
            None,
            c"/tmp",
            Some(c"tmpfs"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            Some(c"mode=1777,size=32m"),
        )?
        .mount_after_pivot_root(
            None,
            c"/run",
            Some(c"tmpfs"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            Some(c"mode=0755,size=16m"),
        )?;

        Ok(self)
    }

    pub fn mount_restricted_proc(&self) -> anyhow::Result<&Self> {
        self.mount_after_pivot_root(
            None,
            c"/proc",
            Some(c"proc"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            Some(c"hidepid=2,subset=pid"),
        )?;

        let user_id: usize = fs::read_to_string("/proc/self/uid_map")?
            .trim()
            .split_once(" ")
            .unwrap_or(("0", ""))
            .0
            .parse()?;
        let group_id: usize = fs::read_to_string("/proc/self/gid_map")?
            .trim()
            .split_once(" ")
            .unwrap_or(("0", ""))
            .0
            .parse()?;
        println!("user_id: {}", user_id);
        println!("group_id: {}", group_id);
        fs::create_dir("/root")?;
        let _ = fs::create_dir("/etc");
        fs::write(
            "/etc/passwd",
            format!("root:x:{}:{}:root:/root:/bash/bash", user_id, group_id),
        )?;
        fs::write("/etc/group", format!("root:x:{}:", group_id))?;
        Ok(self)
    }

    pub fn link_proc_to_dev(&self) -> anyhow::Result<&Self> {
        self.symlink(c"/proc/self/fd", c"/dev/fd")?
            .symlink(c"/proc/self/fd/0", c"/dev/stdin")?
            .symlink(c"/proc/self/fd/1", c"/dev/stdout")?
            .symlink(c"/proc/self/fd/2", c"/dev/stderr")?
            .symlink(c"/proc/kcore", c"/dev/core")?;
        Ok(self)
    }
}
