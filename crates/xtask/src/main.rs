use serde::Deserialize;
use std::{
    collections::BTreeSet,
    env,
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

const UI_TEST_TARGETS: &[&str] = &[
    "-p",
    "riina-ui",
    "-p",
    "riina",
    "--test",
    "headless_visual",
    "--test",
    "quick_settings_input",
];

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if let Err(error) = run_task(&arguments) {
        let _ = writeln!(io::stderr().lock(), "{error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_task(arguments: &[OsString]) -> Result<(), String> {
    // A shared Cargo target can reuse an xtask executable built in a different
    // worktree. Resolve the workspace from the invocation directory.
    let invocation = env::current_dir().map_err(|error| error.to_string())?;
    let workspace = find_workspace(&invocation)?;
    let root = &workspace.workspace_root;
    match arguments {
        [command] if command == "check" => check(root),
        [command, flag] if command == "check" && flag == "--fast" => {
            fast_check(&workspace, OsStr::new("HEAD"))
        }
        [command, flag, option, base]
            if command == "check" && flag == "--fast" && option == "--base" =>
        {
            fast_check(&workspace, base)
        }
        [command] if command == "ui" => ui(root),
        [command] if command == "native" => native(root, None),
        [command, filter] if command == "native" => native(root, Some(filter)),
        [command] if command == "coverage" => coverage(root),
        _ => Err(
            "Usage: cargo xtask {check [--fast [--base <revision>]]|ui|native [filter]|coverage}"
                .into(),
        ),
    }
}

fn check(root: &Path) -> Result<(), String> {
    cargo(root, &["fmt", "--all", "--", "--check"])?;
    python(root, &["scripts/check-line-length.py"])?;
    python(root, &["scripts/check-settings-navigation.py"])?;
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
    script_tests(root)?;
    for script in [
        "scripts/check-parity.py",
        "scripts/check-fixtures.py",
        "scripts/check-assets.py",
    ] {
        python(root, &[script])?;
    }
    Ok(())
}

fn fast_check(workspace: &Workspace, base: &OsStr) -> Result<(), String> {
    let root = &workspace.workspace_root;
    let files = changed_files(root, base)?;
    cargo(root, &["fmt", "--all", "--", "--check"])?;
    python(
        root,
        &[
            OsStr::new("scripts/check-line-length.py"),
            OsStr::new("--baseline"),
            base,
        ],
    )?;
    if files.iter().any(|path| {
        path.starts_with("crates/ui/ui")
            || path == Path::new("crates/app/src/settings_search.rs")
            || path == Path::new("scripts/check-settings-navigation.py")
    }) {
        python(root, &["scripts/check-settings-navigation.py"])?;
    }
    if files
        .iter()
        .any(|path| path.starts_with("scripts") || path.starts_with(".github/workflows"))
    {
        script_tests(root)?;
    }
    if files.iter().any(|path| path.starts_with("assets")) {
        python(root, &["scripts/check-assets.py"])?;
    }
    if files.iter().any(|path| path.starts_with("tests/fixtures")) {
        python(root, &["scripts/check-fixtures.py"])?;
    }
    let workspace_wide = files.iter().any(|path| shared_build_input(path));
    let mut selected = Vec::new();
    let packages;
    if workspace_wide {
        selected.push("--workspace");
    } else {
        let mut changed = workspace.affected_packages(&files);
        if include_ui_consumers(&mut changed, &files) {
            return check_ui(root);
        }
        if changed.is_empty() {
            return if files.is_empty() {
                cargo(root, &["test", "-p", "riina-core", "--locked"])
            } else {
                Ok(())
            };
        }
        packages = changed;
        for package in &packages {
            selected.extend(["-p", package.as_str()]);
        }
    }
    let mut lint = vec!["clippy", "--all-targets", "--locked"];
    lint.extend(&selected);
    lint.extend(["--", "-D", "warnings"]);
    cargo(root, &lint)?;
    let mut tests = vec!["test", "--locked"];
    tests.extend(selected);
    cargo(root, &tests)
}

fn script_tests(root: &Path) -> Result<(), String> {
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
    )
}

fn ui(root: &Path) -> Result<(), String> {
    python(root, &["scripts/check-settings-navigation.py"])?;
    test_ui(root)
}

fn check_ui(root: &Path) -> Result<(), String> {
    let mut lint = vec!["clippy", "--locked", "--lib"];
    lint.extend_from_slice(UI_TEST_TARGETS);
    lint.extend(["--", "-D", "warnings"]);
    cargo(root, &lint)?;
    test_ui(root)
}

fn test_ui(root: &Path) -> Result<(), String> {
    let mut tests = vec!["test", "--locked"];
    tests.extend_from_slice(UI_TEST_TARGETS);
    cargo(root, &tests)
}

fn include_ui_consumers(packages: &mut BTreeSet<String>, files: &BTreeSet<PathBuf>) -> bool {
    if files
        .iter()
        .any(|path| path.starts_with("tests/visual/headless"))
    {
        packages.insert("riina-ui".to_owned());
    }
    let ui_source_changed = files.iter().any(|path| {
        path.starts_with("crates/ui/ui")
            || path.starts_with("crates/ui/src")
            || path == Path::new("crates/ui/build.rs")
            || path == Path::new("crates/ui/Cargo.toml")
    });
    let widget_tests_changed = files.iter().any(|path| is_app_widget_test(path));
    if !ui_source_changed && !widget_tests_changed {
        return false;
    }
    // The widget tests compile the application library. Run the app unit
    // harness as well when its source, build inputs or other tests change.
    if packages
        .iter()
        .all(|package| matches!(package.as_str(), "riina" | "riina-ui"))
        && !files
            .iter()
            .any(|path| path.starts_with("crates/app") && !is_app_widget_test(path))
    {
        return true;
    }
    if ui_source_changed {
        packages.insert("riina".to_owned());
    }
    false
}

fn is_app_widget_test(path: &Path) -> bool {
    path == Path::new("crates/app/tests/quick_settings_input.rs")
        || path.starts_with("crates/app/tests/quick_settings")
        || path.starts_with("crates/app/tests/widgets")
}

fn shared_build_input(path: &Path) -> bool {
    path.starts_with(".cargo")
        || (path.starts_with("assets") && path != Path::new("assets/provenance.json"))
        || path.starts_with("crates/test-support")
        || path.starts_with("tests/fixtures")
        || [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "clippy.toml",
        ]
        .iter()
        .any(|name| path == Path::new(name))
}

fn changed_files(root: &Path, base: &OsStr) -> Result<BTreeSet<PathBuf>, String> {
    let mut files = captured(
        Command::new("git")
            .current_dir(root)
            .args([
                "diff",
                "--name-only",
                "--no-renames",
                "-z",
                "--end-of-options",
            ])
            .arg(base)
            .arg("--"),
    )?;
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
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    path: Option<PathBuf>,
}

impl Workspace {
    fn affected_packages(&self, files: &BTreeSet<PathBuf>) -> BTreeSet<String> {
        let members: Vec<_> = self
            .packages
            .iter()
            .filter(|package| self.workspace_members.contains(&package.id))
            .filter_map(|package| {
                let directory = package.manifest_path.parent()?;
                let relative = directory.strip_prefix(&self.workspace_root).ok()?;
                Some((package, directory, relative))
            })
            .collect();
        let mut selected = BTreeSet::new();
        let mut changed_dependencies = BTreeSet::new();
        for (package, directory, relative) in &members {
            for file in files {
                if let Ok(member_path) = file.strip_prefix(relative) {
                    selected.insert(package.name.clone());
                    if member_path.starts_with("src")
                        || member_path == Path::new("Cargo.toml")
                        || member_path == Path::new("build.rs")
                    {
                        changed_dependencies.insert(*directory);
                    }
                }
            }
        }
        // Slint inputs live outside src and use the focused widget/visual route.
        // Other crate API changes also compile and exercise workspace consumers.
        loop {
            let previous_count = changed_dependencies.len();
            for (package, directory, _) in &members {
                if !changed_dependencies.contains(directory)
                    && package.dependencies.iter().any(|dependency| {
                        dependency
                            .path
                            .as_deref()
                            .is_some_and(|path| changed_dependencies.contains(path))
                    })
                {
                    selected.insert(package.name.clone());
                    changed_dependencies.insert(*directory);
                }
            }
            if changed_dependencies.len() == previous_count {
                break;
            }
        }
        selected
    }
}

fn find_workspace(invocation: &Path) -> Result<Workspace, String> {
    let output = captured(Command::new(cargo_program()).current_dir(invocation).args([
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--locked",
    ]))?;
    let workspace: Workspace = serde_json::from_slice(&output)
        .map_err(|error| format!("Could not read Cargo workspace metadata: {error}"))?;
    let task_manifest = workspace.workspace_root.join("crates/xtask/Cargo.toml");
    if !workspace
        .packages
        .iter()
        .any(|package| package.name == "xtask" && package.manifest_path == task_manifest)
    {
        return Err("Run this command from the Riina workspace".into());
    }
    Ok(workspace)
}

fn native(root: &Path, filter: Option<&OsStr>) -> Result<(), String> {
    let library = env::var_os("RIINA_TEST_MPV_LIBRARY")
        .map(PathBuf::from)
        .or_else(|| cfg!(windows).then(|| root.join("tools/mpv/libmpv-2.dll")))
        .ok_or("Set RIINA_TEST_MPV_LIBRARY to a provisioned libmpv library.")?;
    let library = library
        .canonicalize()
        .map_err(|error| format!("Could not find libmpv at {}: {error}", library.display()))?;
    let threads = env::var_os("RUST_TEST_THREADS").unwrap_or_else(|| {
        // Each engine can start decoder and encoder threads of its own.
        let limit = if cfg!(windows) { 4 } else { 1 };
        std::thread::available_parallelism()
            .map_or(1, |available| available.get().min(limit))
            .to_string()
            .into()
    });
    let mut command = Command::new(cargo_program());
    command
        .current_dir(root)
        .env("RIINA_TEST_MPV_LIBRARY", library)
        .env("RIINA_TEST_MPV_AUDIO_OUTPUT", "null")
        .env("RUST_TEST_THREADS", threads)
        .args([
            "test",
            "--workspace",
            "--test",
            "native",
            "--locked",
            "--",
            "--ignored",
        ]);
    command.args(filter);
    run(&mut command)
}

fn coverage(root: &Path) -> Result<(), String> {
    let tool = Command::new(cargo_program())
        .current_dir(root)
        .args(["llvm-cov", "--version"])
        .output()
        .map_err(|error| format!("Could not check for cargo-llvm-cov: {error}"))?;
    if !tool.status.success() {
        return Err(
            "Coverage requires cargo-llvm-cov and the pinned toolchain's llvm-tools component."
                .into(),
        );
    }
    let target =
        env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), |path| root.join(path));
    let directory = target.join("riina-coverage");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let report = directory.join("workspace.json");
    run(Command::new(cargo_program())
        .current_dir(root)
        .args([
            "llvm-cov",
            "--workspace",
            "--locked",
            "--json",
            "--output-path",
        ])
        .arg(&report))?;
    let executable = if cfg!(windows) { "python" } else { "python3" };
    run(Command::new(executable)
        .current_dir(root)
        .arg("scripts/summarize-coverage.py")
        .arg(report))
}

fn cargo_program() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

fn cargo(root: &Path, arguments: &[&str]) -> Result<(), String> {
    run(Command::new(cargo_program())
        .current_dir(root)
        .args(arguments))
}

fn python(root: &Path, arguments: &[impl AsRef<OsStr>]) -> Result<(), String> {
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

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn ui_source_alone_runs_the_visual_and_interaction_targets() {
        for source in [
            "crates/ui/ui/player.slint",
            "crates/ui/src/lib.rs",
            "crates/ui/build.rs",
            "crates/ui/Cargo.toml",
        ] {
            let mut packages = BTreeSet::from(["riina-ui".to_owned()]);
            assert!(include_ui_consumers(
                &mut packages,
                &BTreeSet::from([source.into()])
            ));
            assert_eq!(packages, BTreeSet::from(["riina-ui".to_owned()]));
        }
    }

    #[test]
    fn ui_and_other_source_changes_include_the_app_in_the_same_batch() {
        let mut packages = BTreeSet::from(["riina-core".to_owned(), "riina-ui".to_owned()]);
        let files = BTreeSet::from([
            PathBuf::from("crates/ui/ui/player.slint"),
            PathBuf::from("crates/core/src/playback.rs"),
        ]);
        assert!(!include_ui_consumers(&mut packages, &files));
        assert_eq!(
            packages,
            BTreeSet::from([
                "riina".to_owned(),
                "riina-core".to_owned(),
                "riina-ui".to_owned()
            ])
        );
    }

    #[test]
    fn widget_test_edits_keep_the_group_focused_until_app_source_changes() {
        let mut packages = BTreeSet::from(["riina".to_owned()]);
        let mut files = BTreeSet::from([
            PathBuf::from("crates/app/tests/quick_settings/workflows.rs"),
            PathBuf::from("crates/app/tests/widgets/settings_navigation.rs"),
        ]);
        assert!(include_ui_consumers(&mut packages, &files));
        files.insert(PathBuf::from("crates/ui/ui/player.slint"));
        packages.insert("riina-ui".to_owned());
        assert!(include_ui_consumers(&mut packages, &files));
        files.insert(PathBuf::from("crates/app/src/player/player_ui.rs"));
        assert!(!include_ui_consumers(&mut packages, &files));
    }

    #[test]
    fn visual_test_and_baseline_edits_stay_in_the_ui_crate() {
        let mut packages = BTreeSet::new();
        let files = BTreeSet::from([
            PathBuf::from("crates/ui/tests/headless_visual.rs"),
            PathBuf::from("tests/visual/headless/windows/player-playing.png"),
        ]);
        assert!(!include_ui_consumers(&mut packages, &files));
        assert_eq!(packages, BTreeSet::from(["riina-ui".to_owned()]));
    }

    #[test]
    fn shared_source_selects_consumers_while_test_edits_stay_local() {
        let task_crate = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace = find_workspace(task_crate).unwrap();
        assert_eq!(workspace.workspace_root.join("crates/xtask"), task_crate);
        for (path, expected) in [
            (
                "crates/core/src/command.rs",
                &["riina", "riina-core", "riina-mpv"][..],
            ),
            ("crates/mpv/src/worker.rs", &["riina", "riina-mpv"][..]),
            ("crates/mpv/tests/native/main.rs", &["riina-mpv"][..]),
        ] {
            let packages = workspace.affected_packages(&BTreeSet::from([path.into()]));
            assert!(
                packages
                    .iter()
                    .map(String::as_str)
                    .eq(expected.iter().copied()),
                "{path}: {packages:?}"
            );
        }
    }

    #[test]
    fn asset_manifest_edits_only_need_asset_validation() {
        assert!(!shared_build_input(Path::new("assets/provenance.json")));
        assert!(shared_build_input(Path::new("assets/icons/chapters.svg")));
    }
}
