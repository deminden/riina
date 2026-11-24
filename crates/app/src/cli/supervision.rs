//! Stop only the player started by a waiting launcher when the terminal interrupts it.
use std::{
    io,
    process::{Child, Command, ExitCode},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);
const CHECK_INTERVAL: Duration = Duration::from_millis(50);

pub(super) struct Supervisor {
    #[cfg(windows)]
    job: crate::platform::windows_job::KillOnCloseJob,
}

impl Supervisor {
    pub(super) fn new() -> io::Result<Self> {
        install_interrupt_handlers()?;
        Ok(Self {
            #[cfg(windows)]
            job: crate::platform::windows_job::KillOnCloseJob::new()?,
        })
    }

    pub(super) fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        #[cfg(unix)]
        {
            command.spawn()
        }
        #[cfg(windows)]
        {
            self.job.spawn(command)
        }
    }

    pub(super) fn wait(&self, mut child: Child) -> io::Result<ExitCode> {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code().and_then(|code| u8::try_from(code).ok());
                    return Ok(code.map_or(ExitCode::FAILURE, ExitCode::from));
                }
                Ok(None) if !INTERRUPTED.load(Ordering::Relaxed) => {
                    thread::sleep(CHECK_INTERVAL);
                }
                Ok(None) => {
                    self.stop(&mut child);
                    return Ok(ExitCode::FAILURE);
                }
                Err(error) => {
                    self.stop(&mut child);
                    return Err(error);
                }
            }
        }
    }

    #[cfg(unix)]
    fn stop(&self, child: &mut Child) {
        if let Ok(pid) = libc::pid_t::try_from(child.id()) {
            // SAFETY: wait has not reaped this child and its PID is still reserved.
            if unsafe { libc::kill(pid, libc::SIGTERM) } == 0 {
                for _ in 0..10 {
                    if matches!(child.try_wait(), Ok(Some(_))) {
                        return;
                    }
                    thread::sleep(CHECK_INTERVAL);
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }

    #[cfg(windows)]
    fn stop(&self, child: &mut Child) {
        self.job.terminate();
        let _ = child.wait();
    }
}

#[cfg(unix)]
pub(super) fn configure(command: &mut Command, reads_stdin: bool) {
    use std::{io::IsTerminal, os::unix::process::CommandExt};
    // An explicitly selected terminal must remain readable by the foreground group.
    if !reads_stdin || !io::stdin().is_terminal() {
        command.process_group(0);
    }
}

#[cfg(windows)]
pub(super) fn configure(command: &mut Command, _reads_stdin: bool) {
    use std::os::windows::process::CommandExt;
    use windows::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP.0);
}

#[cfg(unix)]
fn install_interrupt_handlers() -> io::Result<()> {
    extern "C" fn interrupt(_signal: libc::c_int) {
        INTERRUPTED.store(true, Ordering::Relaxed);
    }
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        // SAFETY: the process-lifetime handler performs only a lock-free atomic store.
        if unsafe { libc::signal(signal, interrupt as *const () as libc::sighandler_t) }
            == libc::SIG_ERR
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn install_interrupt_handlers() -> io::Result<()> {
    use windows::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
    };
    use windows::core::BOOL;

    unsafe extern "system" fn interrupt(event: u32) -> BOOL {
        let handled = matches!(event, CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT);
        if handled {
            INTERRUPTED.store(true, Ordering::Relaxed);
        }
        handled.into()
    }

    // SAFETY: the process-lifetime callback uses only the atomic interruption flag.
    unsafe { SetConsoleCtrlHandler(Some(interrupt), true) }.map_err(Into::into)
}
