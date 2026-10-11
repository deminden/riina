"""Regression tests for license-grant selection in native package sources."""
import importlib.util
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
