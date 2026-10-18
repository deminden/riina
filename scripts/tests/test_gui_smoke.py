"""The package smoke must reject partial, stale, or falsely typed evidence."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("smoke", Path(__file__).parents[1] / "check-gui-smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class GuiSmokeTests(unittest.TestCase):
    def report(self):
        return {"success": True, "pid": 42, "executable": str(Path("package/riina.exe").resolve()),
                "title": "fixture.avi — riina", "checks": sorted(smoke.CHECKS), "physical_display_verified": False}

    def check(self, report):
        smoke.validate(report, "package/riina.exe", 42, "fixture.avi")

    def test_complete_report_from_exact_executable_is_accepted(self):
        self.check(self.report())

    def test_success_must_be_a_boolean_and_process_must_match(self):
        for field, value in [("success", "false"), ("success", 1), ("pid", 43), ("pid", True),
                             ("executable", "development/riina.exe"), ("title", "other.avi — riina")]:
            with self.subTest(field=field, value=value):
                report = self.report()
                report[field] = value
                with self.assertRaises(ValueError):
                    self.check(report)

    def test_missing_duplicate_or_physical_display_claim_fails(self):
        for checks in [[], sorted(smoke.CHECKS)[:-1], [sorted(smoke.CHECKS)[0]] * len(smoke.CHECKS), [None]]:
            with self.subTest(checks=checks):
                report = self.report()
                report["checks"] = checks
                with self.assertRaises(ValueError):
                    self.check(report)
        report = self.report()
        report["physical_display_verified"] = True
        with self.assertRaises(ValueError):
            self.check(report)


if __name__ == "__main__":
    unittest.main()
