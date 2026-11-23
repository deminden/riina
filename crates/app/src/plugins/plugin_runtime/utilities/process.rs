//! A supervisor drains both pipes while the realm and the player remain responsive.
use super::process_platform::{OutputPipe, ProcessGroup};
use encoding_rs::{Decoder, UTF_8};
use serde::Serialize;
use std::{
    io,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: usize = 1024 * 1024;
const IDLE_WAIT: Duration = Duration::from_millis(12);

pub(super) struct Invocation {
    pub(super) executable: PathBuf,
    pub(super) args: Vec<String>,
    pub(super) directory: PathBuf,
}

#[derive(Serialize)]
pub(in crate::plugins::plugin_runtime) struct Completed {
    status: i32,
    stdout: String,
    stderr: String,
}

#[derive(Clone, Copy)]
pub(in crate::plugins::plugin_runtime) enum Channel {
    Stdout,
    Stderr,
}

impl Invocation {
    pub(super) fn run(
        &self,
        deadline: Instant,
        cancelled: impl Fn() -> bool,
        mut output: impl FnMut(Channel, String) -> bool,
    ) -> io::Result<Completed> {
        active(&cancelled, deadline)?;
        let mut command = Command::new(&self.executable);
        command
            .args(&self.args)
            .current_dir(&self.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .env(
                "LC_ALL",
                if cfg!(target_os = "linux") {
                    "C.UTF-8"
                } else {
                    "en_US.UTF-8"
                },
            );
        for name in ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP", "TMPDIR"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let (child, group) = ProcessGroup::spawn(&mut command)?;
        let mut guard = Running {
            child,
            group,
            reaped: false,
        };
        let mut stdout = PipeReader::new(guard.child.stdout.take())?;
        let mut stderr = PipeReader::new(guard.child.stderr.take())?;
        loop {
            active(&cancelled, deadline)?;
            let reading_stdout = stdout.read(|text| output(Channel::Stdout, text))?;
            let reading_stderr = stderr.read(|text| output(Channel::Stderr, text))?;
            if stdout.closed
                && stderr.closed
                && let Some(status) = guard.child.try_wait()?
            {
                guard.reaped = true;
                return Ok(Completed {
                    status: exit_code(status),
                    stdout: stdout.text,
                    stderr: stderr.text,
                });
            }
            if !reading_stdout && !reading_stderr {
                std::thread::sleep(
                    IDLE_WAIT.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
        }
    }
}

fn active(cancelled: &impl Fn() -> bool, deadline: Instant) -> io::Result<()> {
    if cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Plugin session closed",
        ));
    }
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Plugin helper timed out",
        ));
    }
    Ok(())
}

fn exit_code(status: ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.code().or_else(|| status.signal()).unwrap_or(-1)
    }
    #[cfg(windows)]
    {
        status.code().unwrap_or(-1)
    }
}

struct Running {
    child: Child,
    group: ProcessGroup,
    reaped: bool,
}

impl Drop for Running {
    fn drop(&mut self) {
        if !self.reaped {
            // Terminate the original POSIX group before a wait can recycle its
            // leader's PID, including when an exited helper left open pipes.
            self.group.terminate(&mut self.child);
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct PipeReader<P> {
    pipe: P,
    decoder: Decoder,
    bytes_read: usize,
    text: String,
    closed: bool,
}

impl<P: OutputPipe> PipeReader<P> {
    fn new(pipe: Option<P>) -> io::Result<Self> {
        let pipe = pipe.ok_or_else(|| io::Error::other("Missing helper output pipe"))?;
        pipe.prepare()?;
        Ok(Self {
            pipe,
            decoder: UTF_8.new_decoder_without_bom_handling(),
            bytes_read: 0,
            text: String::new(),
            closed: false,
        })
    }

    fn read(&mut self, output: impl FnOnce(String) -> bool) -> io::Result<bool> {
        if self.closed {
            return Ok(false);
        }
        let mut bytes = [0_u8; 8192];
        let count = match self.pipe.read_available(&mut bytes) {
            Ok(count) => count,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        self.closed = count == 0;
        self.bytes_read += count;
        if self.bytes_read > OUTPUT_LIMIT {
            return Err(io::Error::other(
                "Plugin helper output exceeded 1 MiB on one stream",
            ));
        }
        // Windows and POSIX can split a UTF-8 character across reads.
        let mut chunk = String::with_capacity(count * 3 + 12);
        let _ = self
            .decoder
            .decode_to_string(&bytes[..count], &mut chunk, self.closed);
        if !chunk.is_empty() {
            self.text.push_str(&chunk);
            if !output(chunk) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Plugin session closed",
                ));
            }
        }
        Ok(count > 0)
    }
}
