// The player is a GUI in debug builds too. Leaving the debug executable as a
// console-subsystem process creates a second black conhost window behind it.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    // SAFETY: the binary enters startup on its main thread before creating any workers.
    unsafe { riina_app::run_from_command_line() }
}
