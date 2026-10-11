"""Regression tests for license-grant selection in native package sources."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "package-rust-licenses.py"
SPEC = importlib.util.spec_from_file_location("package_rust_licenses", SCRIPT)
LICENSES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LICENSES)


class LicenseSelectionTests(unittest.TestCase):
    def test_unicode_data_and_software_license_is_accepted_with_notice(self):
        selected = LICENSES.choose_license("Unicode-DFS-2016")

        self.assertEqual(selected, "Unicode-DFS-2016")
        self.assertIn(selected, LICENSES.GPLV3_COMPATIBLE_GRANTS)

    def test_license_is_shared_from_a_sibling_crate_at_the_same_git_revision(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repository = "https://github.com/example/rquickjs"
            revision = "d7ef5eeae702fea24c03643064de454f1c1dd4b0"
            sibling_root = root / "rquickjs"
            package_root = root / "core"
            sibling_root.mkdir()
            package_root.mkdir()
            license_text = "MIT license text from the pinned monorepo revision.\n"
            (sibling_root / "LICENSE").write_text(license_text, encoding="utf-8")
            (sibling_root / ".cargo_vcs_info.json").write_text(
                json.dumps({"git": {"sha1": revision}, "path_in_vcs": ""}),
                encoding="utf-8",
            )
            (package_root / ".cargo_vcs_info.json").write_text(
                json.dumps({"git": {"sha1": revision}, "path_in_vcs": "core"}),
                encoding="utf-8",
            )
            sibling = {
                "id": "registry+rquickjs",
                "name": "rquickjs",
                "license": "MIT",
                "repository": repository,
                "manifest_path": str(sibling_root / "Cargo.toml"),
            }
            package = {
                "id": "registry+rquickjs-core",
                "name": "rquickjs-core",
                "license": "MIT",
                "repository": repository,
                "manifest_path": str(package_root / "Cargo.toml"),
            }
            (sibling_root / "Cargo.toml").touch()
            (package_root / "Cargo.toml").touch()
            target = root / "output" / "packages" / "rquickjs-core-0.14.0"
            records = LICENSES.copy_sibling_notice_files(
                package, [sibling, package], target, root / "output"
            )

            self.assertEqual(len(records), 1)
            copied = root / "output" / records[0]["path"]
            self.assertEqual(copied.read_text(encoding="utf-8"), license_text)
            self.assertEqual(
                records[0]["source_url"],
                f"https://raw.githubusercontent.com/example/rquickjs/{revision}/LICENSE",
            )


if __name__ == "__main__":
    unittest.main()
