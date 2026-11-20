"""Keep the full Linux app's native dependencies in the port build gate."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ApplicationPortsTests(unittest.TestCase):
    def test_linux_build_provisions_and_probes_both_webview_libraries(self):
        source = (ROOT / ".github/workflows/application-ports.yml").read_text(encoding="utf-8")
        setup = source.split("- name: Linux GUI build prerequisites", 1)[1].split("- name:", 1)[0]
        packages = re.search(r"apt-get install -y ([^\n]+)", setup).group(1).split()
        self.assertTrue({"libgtk-3-dev", "libwebkit2gtk-4.1-dev", "pkg-config"} <= set(packages))
        self.assertIn("pkg-config --modversion gtk+-3.0 webkit2gtk-4.1", setup)
        self.assertLess(source.index("pkg-config --modversion"), source.index("cargo clippy"))

    def test_ports_build_and_test_the_application_as_well_as_libraries(self):
        source = (ROOT / ".github/workflows/application-ports.yml").read_text(encoding="utf-8")
        for command in ("cargo clippy --workspace --all-targets --locked -- -D warnings",
                        "cargo test --workspace --locked", "cargo build -p riina --locked"):
            self.assertIn(command, source)
        self.assertIn("package-macos-app.py", source)
        self.assertIn("plutil -lint", source)

    def test_application_jobs_share_no_duplicate_core_workflow(self):
        self.assertFalse((ROOT / ".github/workflows/core.yml").exists())
        for name in ("windows.yml", "application-ports.yml"):
            source = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
            self.assertIn("actions/cache/restore@", source)
            self.assertIn("actions/cache/save@", source)
            self.assertIn("hashFiles('rust-toolchain.toml', 'Cargo.lock'", source)
            self.assertIn("cargo test --workspace --locked", source)

    def test_windows_native_push_job_uses_the_verified_development_archive(self):
        source = (ROOT / ".github/workflows/windows.yml").read_text(encoding="utf-8")
        native = source.split("\n  native:\n", 1)[1].split("\n  advisories:\n", 1)[0]
        self.assertIn("if: github.event_name == 'push'", native)
        self.assertIn("hashFiles('scripts/fetch-mpv.ps1')", native)
        self.assertLess(native.index("./scripts/fetch-mpv.ps1"), native.index("cargo xtask native"))
        self.assertIn("GUI, HDR, hardware decoding and audible output require separate evidence", native)


if __name__ == "__main__":
    unittest.main()
