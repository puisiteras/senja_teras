extern "C" fn child_entry<F>(arg: *mut libc::c_void) -> libc::c_int
where
    F: FnMut() -> anyhow::Result<()>,
{
    let closure = unsafe { &mut *(arg as *mut F) };
    match closure() {
        Ok(_) => unreachable!(),
        Err(err) => {
            eprintln!("child handler error:\n{:?}\n\nexitting...", err);
            -1
        }
    }
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

        let arg_ptr = &mut handler as *mut F as *mut libc::c_void;

        let clone_flags = self.nsflag | libc::SIGCHLD;

        let pid = unsafe { libc::clone(child_entry::<F>, aligned_stack_top, clone_flags, arg_ptr) };

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
