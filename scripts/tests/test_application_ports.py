"""Keep the full Linux app's native dependencies in the port build gate."""
import plistlib
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

from workflow_reader import named_step, workflow

ROOT = Path(__file__).resolve().parents[2]


class ApplicationPortsTests(unittest.TestCase):
    def test_linux_build_provisions_and_probes_both_webview_libraries(self):
        job = workflow("application-ports.yml")["jobs"]["application"]
        setup = named_step(job, "Linux GUI build prerequisites")
        self.assertEqual(setup["if"], "runner.os == 'Linux'")
        packages = re.search(r"apt-get install -y (.+)", setup["run"]).group(1).split()
        self.assertTrue({"libgtk-3-dev", "libwebkit2gtk-4.1-dev", "pkg-config"} <= set(packages))
        self.assertIn("pkg-config --modversion gtk+-3.0 webkit2gtk-4.1", setup["run"])
        lint = named_step(job, "Full application lint")
        self.assertLess(job["steps"].index(setup), job["steps"].index(lint))

    def test_ports_build_and_test_the_application_as_well_as_libraries(self):
        job = workflow("application-ports.yml")["jobs"]["application"]
        for name, command in (
            ("Full application lint", "cargo clippy --workspace --all-targets --locked -- -D warnings"),
            ("Application and shared unit tests", "cargo test --workspace --locked"),
            ("Full application build", "cargo build -p riina --locked"),
        ):
            self.assertEqual(named_step(job, name)["run"], command)
        bundle = named_step(job, "Assemble and validate the macOS app bundle")
        self.assertEqual(bundle["if"], "runner.os == 'macOS'")
        self.assertIn("package-macos-app.py", bundle["run"])
        self.assertIn("plutil -lint", bundle["run"])

    def test_browser_setup_is_bundled_with_or_without_url_registration(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = root / "riina"
            executable.write_bytes(b"application fixture")
            launcher = root / "riina-cli"
            launcher.write_bytes(b"terminal fixture")
            extension = ROOT / "browser" / "open-in-riina"
            for opt_in, flags in enumerate(([], ["--browser-links"])):
                with self.subTest(browser_links=bool(opt_in)):
                    bundle = root / f"Riina-{opt_in}.app"
                    subprocess.run(
                        [
                            sys.executable, ROOT / "scripts" / "package-macos-app.py",
                            executable, bundle, *flags,
                        ],
                        check=True,
                        capture_output=True,
                    )
                    contents = bundle / "Contents"
                    self.assertEqual(
                        (contents / "MacOS" / "riina-cli").read_bytes(), launcher.read_bytes()
                    )
                    with (contents / "Info.plist").open("rb") as info:
                        metadata = plistlib.load(info)
                    if opt_in:
                        self.assertEqual(metadata["CFBundleURLTypes"][0]["CFBundleURLSchemes"], ["riina"])
                    else:
                        self.assertNotIn("CFBundleURLTypes", metadata)
                    resources = contents / "Resources" / "browser-extension"
                    self.assertEqual(
                        {item.name: item.read_bytes() for item in resources.iterdir()},
                        {item.name: item.read_bytes() for item in extension.iterdir()},
                    )

    def test_application_jobs_share_no_duplicate_core_workflow(self):
        self.assertFalse((ROOT / ".github/workflows/core.yml").exists())
        for name, key, unit_tests in (
            ("windows.yml", "windows", "Domain tests"),
            ("application-ports.yml", "application", "Application and shared unit tests"),
        ):
            job = workflow(name)["jobs"][key]
            restore = named_step(job, "Restore Rust dependency and compiler outputs")
            save = named_step(job, "Preserve compiler outputs")
            self.assertTrue(restore["uses"].startswith("actions/cache/restore@"))
            self.assertTrue(save["uses"].startswith("actions/cache/save@"))
            self.assertIn("hashFiles('rust-toolchain.toml', 'Cargo.lock'", restore["with"]["key"])
            self.assertIn("cargo test --workspace --locked", named_step(job, unit_tests)["run"])

    def test_windows_native_push_job_uses_the_verified_development_archive(self):
        job = workflow("windows.yml")["jobs"]["native"]
        self.assertEqual(job["if"], "github.event_name == 'push'")
        cache = named_step(job, "Restore the pinned development runtime archive")
        self.assertIn("hashFiles('scripts/fetch-mpv.ps1')", cache["with"]["key"])
        unpack = named_step(job, "Verify and unpack the pinned development DLL")
        tests = named_step(job, "Native engine tests with null output")
        self.assertIn("./scripts/fetch-mpv.ps1", unpack["run"])
        self.assertIn("cargo xtask native", tests["run"])
        self.assertLess(job["steps"].index(unpack), job["steps"].index(tests))
        scope = named_step(job, "Verification scope")
        self.assertIn("GUI, HDR, hardware decoding and audible output require separate evidence", scope["run"])

    def test_windows_checks_settings_routes_before_rust_compilation(self):
        job = workflow("windows.yml")["jobs"]["windows"]
        integrity = named_step(job, "Reference integrity and traceability")
        self.assertIn("python scripts/check-settings-navigation.py", integrity["run"].splitlines())
        lint = named_step(job, "Lint")
        self.assertLess(job["steps"].index(integrity), job["steps"].index(lint))


if __name__ == "__main__":
    unittest.main()
