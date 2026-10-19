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


if __name__ == "__main__":
    unittest.main()
