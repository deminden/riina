use serde::Deserialize;
use std::{
    collections::BTreeSet,
    env,
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let result = match arguments.as_slice() {
        [command] if command == "check" => check(&root),
        [command, flag] if command == "check" && flag == "--fast" => fast_check(&root),
        [command] if command == "native" => native(&root),
        _ => Err("Usage: cargo xtask {check [--fast]|native}".into()),
    };
    if let Err(error) = result {
        let _ = writeln!(io::stderr().lock(), "{error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn check(root: &Path) -> Result<(), String> {
    cargo(root, &["fmt", "--all", "--", "--check"])?;
    python(root, &["scripts/check-line-length.py"])?;
    cargo(
        root,
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    cargo(root, &["test", "--workspace", "--locked"])?;
    python(
        root,
        &[
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts/tests",
            "-p",
            "test_*.py",
        ],
    )?;
    for script in [
        "scripts/check-parity.py",
        "scripts/check-fixtures.py",
        "scripts/check-assets.py",
    ] {
        python(root, &[script])?;
    }
    Ok(())
}

fn fast_check(root: &Path) -> Result<(), String> {
    cargo(root, &["fmt", "--all", "--", "--check"])?;
    let files = changed_files(root)?;
    let workspace_wide = files.iter().any(|path| shared_build_input(path));
    let selected = if workspace_wide {
        BTreeSet::new()
    } else {
        changed_packages(root, &files)?
    };
    let mut args = vec!["clippy", "--all-targets", "--locked"];
    if workspace_wide {
        args.push("--workspace");
    } else {
        if selected.is_empty() {
            return core_tests(root);
        }
        for package in &selected {
            args.extend(["-p", package.as_str()]);
        }
    }
    args.extend(["--", "-D", "warnings"]);
    cargo(root, &args)?;
    core_tests(root)
}

fn core_tests(root: &Path) -> Result<(), String> {
    cargo(root, &["test", "-p", "riina-core", "--locked"])
}

fn shared_build_input(path: &Path) -> bool {
    path.starts_with(".cargo")
        || path.starts_with("assets")
        || [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "clippy.toml",
        ]
        .iter()
        .any(|name| path == Path::new(name))
}

fn changed_files(root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let mut files = captured(Command::new("git").current_dir(root).args([
        "diff",
        "--name-only",
        "--no-renames",
        "-z",
        "HEAD",
        "--",
    ]))?;
    files.extend(captured(Command::new("git").current_dir(root).args([
        "ls-files",
        "--others",
        "--exclude-standard",
        "-z",
    ]))?);
    let names = String::from_utf8(files)
        .map_err(|error| format!("Invalid UTF-8 path from Git: {error}"))?;
    Ok(names
        .split('\0')
        .filter(|name| !name.is_empty())
        .map(PathBuf::from)
        .collect())
}

#[derive(Deserialize)]
struct Workspace {
    workspace_root: PathBuf,
    workspace_members: BTreeSet<String>,
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
}

fn changed_packages(root: &Path, files: &BTreeSet<PathBuf>) -> Result<BTreeSet<String>, String> {
    if files.is_empty() {
        return Ok(BTreeSet::new());
    }
    let output = captured(Command::new(cargo_program()).current_dir(root).args([
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--locked",
    ]))?;
    let workspace: Workspace = serde_json::from_slice(&output)
        .map_err(|error| format!("Could not read Cargo workspace metadata: {error}"))?;
    Ok(workspace
        .packages
        .into_iter()
        .filter(|package| workspace.workspace_members.contains(&package.id))
        .filter(|package| {
            package
                .manifest_path
                .parent()
                .and_then(|directory| directory.strip_prefix(&workspace.workspace_root).ok())
                .is_some_and(|directory| files.iter().any(|file| file.starts_with(directory)))
        })
        .map(|package| package.name)
        .collect())
}

fn native(root: &Path) -> Result<(), String> {
    let library = env::var_os("RIINA_TEST_MPV_LIBRARY")
        .map(PathBuf::from)
        .or_else(|| cfg!(windows).then(|| root.join("tools/mpv/libmpv-2.dll")))
        .ok_or("Set RIINA_TEST_MPV_LIBRARY to a provisioned libmpv library.")?;
    let library = library
        .canonicalize()
        .map_err(|error| format!("Could not find libmpv at {}: {error}", library.display()))?;
    run(Command::new(cargo_program())
        .current_dir(root)
        .env("RIINA_TEST_MPV_LIBRARY", library)
        .env("RIINA_TEST_MPV_AUDIO_OUTPUT", "null")
        .args([
            "test",
            "-p",
            "riina-mpv",
            "--test",
            "native",
            "--locked",
            "--",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ]))
}

fn cargo_program() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

fn cargo(root: &Path, arguments: &[&str]) -> Result<(), String> {
    run(Command::new(cargo_program())
        .current_dir(root)
        .args(arguments))
}

fn python(root: &Path, arguments: &[&str]) -> Result<(), String> {
    let executable = if cfg!(windows) { "python" } else { "python3" };
    run(Command::new(executable).current_dir(root).args(arguments))
}

fn run(command: &mut Command) -> Result<(), String> {
    let invocation = command
        .get_args()
        .map(OsStr::to_string_lossy)
        .collect::<Vec<_>>()
        .join(" ");
    writeln!(
        io::stderr().lock(),
        "> {} {invocation}",
        command.get_program().to_string_lossy()
    )
    .map_err(|error| error.to_string())?;
    let status = command
        .status()
        .map_err(|error| format!("Could not run {command:?}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Check failed ({status}): {invocation}"))
    }
}

fn captured(command: &mut Command) -> Result<Vec<u8>, String> {
    let output = command
        .output()
        .map_err(|error| format!("Could not run {command:?}: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{command:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
