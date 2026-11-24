//! Console entry point for launching the adjacent windowed player.
mod supervision;

use crate::startup_arguments::StartupArguments;
use std::{
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
};

const HELP: &str = "Usage: riina-cli [options] [files or URLs] [-- mpv-options]

  -h, --help            Show this help.
  -w, --separate-windows
                        Open each file in its own player window.
  --stdin, -            Play media from standard input.
  --no-stdin            Use the listed files even when input is redirected.
  --keep-running        Wait for the player to exit.
  --music-mode          Open the last player in Music Mode.
  --pip                 Open the last player in Picture in Picture.
  --mpv-name=value      Apply an mpv option, such as --mpv-volume=40.
  -- mpv-options        Pass raw mpv options, such as -- --speed=1.25.

Riina automatically uses redirected input when data is ready at launch.
Use --stdin for a producer that may send its first bytes later.
Standard input takes precedence over file arguments and keeps the launcher running.
";

/// Run the console launcher. The normal application entry point owns the GUI.
pub fn run_cli() -> ExitCode {
    match run(std::env::args_os().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "riina-cli: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<OsString>) -> Result<ExitCode, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        io::stdout()
            .lock()
            .write_all(HELP.as_bytes())
            .map_err(|error| error.to_string())?;
        return Ok(ExitCode::SUCCESS);
    }

    let (startup, browser) = StartupArguments::invocation(args.iter().cloned())?;
    let player = adjacent_player().map_err(|error| error.to_string())?;
    let mut command = Command::new(&player);
    if browser.is_none() {
        // Keep the choice made in the terminal even if stdin changes before the GUI starts.
        command.arg(if startup.reads_stdin() {
            "--stdin"
        } else {
            "--no-stdin"
        });
    }
    command.args(args).stdin(if startup.reads_stdin() {
        Stdio::inherit()
    } else {
        Stdio::null()
    });
    supervision::configure(&mut command, startup.reads_stdin());

    if startup.waits_for_player() {
        let supervisor = supervision::Supervisor::new().map_err(|error| error.to_string())?;
        let child = supervisor
            .spawn(&mut command)
            .map_err(|error| launch_error(&player, error))?;
        return supervisor.wait(child).map_err(|error| error.to_string());
    }

    command.stdout(Stdio::null()).stderr(Stdio::null());
    command
        .spawn()
        .map_err(|error| launch_error(&player, error))?;
    Ok(ExitCode::SUCCESS)
}

fn adjacent_player() -> io::Result<PathBuf> {
    let launcher = std::env::current_exe()?.canonicalize()?;
    let player = if cfg!(windows) { "riina.exe" } else { "riina" };
    Ok(launcher.with_file_name(player))
}

fn launch_error(player: &std::path::Path, error: io::Error) -> String {
    let name = player.file_name().unwrap_or_else(|| OsStr::new("riina"));
    if error.kind() == io::ErrorKind::NotFound {
        return format!(
            "Keep {} beside the launcher in the Riina installation.",
            name.display()
        );
    }
    format!("Could not start {}: {error}", player.display())
}
