use std::fs;

struct ChildArgs<'a, F> {
    handler: &'a mut F,
    read_pipe: libc::c_int,
}

extern "C" fn child_entry<F>(arg: *mut libc::c_void) -> libc::c_int
where
    F: FnMut() -> anyhow::Result<()>,
{
    let args = unsafe { &mut *(arg as *mut ChildArgs<F>) };

    let mut buf = [0u8; 1];
    unsafe {
        libc::read(args.read_pipe, buf.as_mut_ptr() as *mut libc::c_void, 1);
        libc::close(args.read_pipe);
    };

    if let Err(e) = (args.handler)() {
        eprintln!("child handler error:\n{:?}", e);
        return 255;
    }
    0
}

pub struct NewNamespace {
    nsflag: libc::c_int,
}

impl NewNamespace {
    pub fn new(nsflag: libc::c_int) -> Self {
        Self { nsflag }
    }

    pub fn run<F>(&self, mut handler: F) -> anyhow::Result<()>
    where
        F: FnMut() -> anyhow::Result<()>,
    {
        let stack_size = 2 * 1024 * 1024;
        let mut stack = vec![0u8; stack_size];
        let stack_ptr = stack.as_mut_ptr();

        let aligned_stack_top = unsafe {
            // Geser pointer ke ujung akhir array
            let stack_end = stack_ptr.add(stack_size);
            // Lakukan bitwise AND untuk memastikan memory ter-align 16-byte (wajib buat C-ABI)
            ((stack_end as usize) & !0xF) as *mut libc::c_void
        };

        let mut pipefd = [-1; 2];
        unsafe {
            if libc::pipe(pipefd.as_mut_ptr()) < 0 {
                return Err(anyhow::anyhow!("Fail create pipe"));
            }
        }

        let mut child_args = ChildArgs {
            handler: &mut handler,
            read_pipe: pipefd[0],
        };
        let arg_ptr = &mut child_args as *mut ChildArgs<F> as *mut libc::c_void;

        let clone_flags = self.nsflag | libc::SIGCHLD;

        let pid = unsafe { libc::clone(child_entry::<F>, aligned_stack_top, clone_flags, arg_ptr) };

        if pid < 0 {
            return Err(anyhow::anyhow!("Clone failed"));
        }

        unsafe { libc::close(pipefd[0]) };

        if (self.nsflag & libc::CLONE_NEWUSER) != 0 {
            fs::write(format!("/proc/{}/uid_map", pid), "0 0 1\n")?;
            fs::write(format!("/proc/{}/setgroups", pid), "deny\n")?;
            fs::write(format!("/proc/{}/gid_map", pid), "0 0 1\n")?;
        }

        let buf = [1u8; 1];
        unsafe {
            libc::write(pipefd[1], buf.as_ptr() as *const libc::c_void, 1);
            libc::close(pipefd[1]); // Tutup write end setelah ngirim sinyal
        }
        println!("mapping uid & gid done.");

        let mut status: libc::c_int = 0;
        let w = unsafe { libc::waitpid(pid, &mut status as *mut _, 0) };
        if w < 0 {
            return Err(anyhow::anyhow!(
                "waitpid error: {:?}",
                std::io::Error::last_os_error()
            ));
        }

        if libc::WIFEXITED(status) {
            let code = libc::WEXITSTATUS(status);
            eprintln!("child exited with code {code}");
        } else if libc::WIFSIGNALED(status) {
            let sig = libc::WTERMSIG(status);
            eprintln!("child killed by signal {sig}");
        }
        Ok(())
    }
}
