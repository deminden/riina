use std::process::Command;

#[test]
fn terminal_help_exits_without_opening_the_application() {
    for alias in ["--help", "-h"] {
        let output = Command::new(env!("CARGO_BIN_EXE_riina-cli"))
            .arg(alias)
            .output()
            .unwrap();
        assert!(output.status.success());
        let help = std::str::from_utf8(&output.stdout).unwrap();
        assert!(help.starts_with("Usage: riina-cli"));
        assert!(help.contains("--keep-running"));
        assert!(help.contains("--no-stdin"));
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn terminal_argument_errors_are_written_to_stderr() {
    let output = Command::new(env!("CARGO_BIN_EXE_riina-cli"))
        .args(["--pip", "--music-mode"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = std::str::from_utf8(&output.stderr).unwrap();
    assert!(error.contains("Music Mode and Picture in Picture cannot be combined"));
}
