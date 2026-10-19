"""The package smoke must reject partial, stale, or falsely typed evidence."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("smoke", Path(__file__).parents[1] / "check-gui-smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class GuiSmokeTests(unittest.TestCase):
    def report(self):
        return {"success": True, "pid": 42, "executable": str(Path("package/riina.exe").resolve()),
                "error": None, "elapsed_ms": 6034,
                "title": "fixture.avi — riina", "checks": sorted(smoke.CHECKS), "physical_display_verified": False}

    def check(self, report):
        smoke.validate(report, "package/riina.exe", 42, "fixture.avi")

    def test_complete_report_from_exact_executable_is_accepted(self):
        self.check(self.report())

    def test_every_required_field_and_check_is_enforced_individually(self):
        for field in self.report():
            with self.subTest(missing_field=field):
                report = self.report()
                del report[field]
                with self.assertRaises(ValueError):
                    self.check(report)
        for name in smoke.CHECKS:
            with self.subTest(missing_check=name):
                report = self.report()
                report["checks"].remove(name)
                with self.assertRaises(ValueError):
                    self.check(report)

    def test_malformed_root_and_contradictory_success_are_rejected(self):
        for value in [None, [], "success", 1, True]:
            with self.subTest(root=value), self.assertRaises(ValueError):
                self.check(value)
        report = self.report()
        report["error"] = "decoder shutdown failed"
        with self.assertRaisesRegex(ValueError, "no error"):
            self.check(report)

    def test_duration_is_a_bounded_integer_and_title_is_the_exact_fixture(self):
        for field, values in {
            "elapsed_ms": [None, False, 0, -1, 40_001, 6.5, "6034", float("inf"), float("nan")],
            "title": [None, ["fixture.avi"], "other-fixture.avi — riina", "fixture.avi.bak — riina"],
        }.items():
            for value in values:
                with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                    report = self.report()
                    report[field] = value
                    self.check(report)
        for elapsed in [1, 40_000]:
            report = self.report()
            report["elapsed_ms"] = elapsed
            self.check(report)

    def test_equivalent_executable_path_is_accepted_but_zero_process_is_not(self):
        report = self.report()
        report["executable"] = str(Path("package/nested/../riina.exe"))
        self.check(report)
        for process in [0, -1, True]:
            with self.subTest(process=process), self.assertRaises(ValueError):
                report["pid"] = process
                smoke.validate(report, "package/riina.exe", process, "fixture.avi")

    def test_check_order_does_not_matter_but_unknown_replacements_fail(self):
        report = self.report()
        report["checks"].reverse()
        self.check(report)
        report["checks"][0] = "unknown_check"
        with self.assertRaises(ValueError):
            self.check(report)

    def test_command_line_gate_exits_nonzero_for_bad_json_and_invalid_evidence(self):
        # Exercise the actual release entry point, not only its imported helper.
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "report.json"
            command = [sys.executable, str(Path(smoke.__file__)), str(path),
                       "--executable", "package/riina.exe", "--pid", "42", "--fixture", "fixture.avi"]
            for payload, accepted in [(json.dumps(self.report()), True), ("{", False),
                                      ("[]", False), (json.dumps({"success": True}), False)]:
                with self.subTest(payload=payload):
                    path.write_text(payload, encoding="utf-8")
                    result = subprocess.run(command, capture_output=True, text=True, timeout=5)
                    self.assertEqual(result.returncode == 0, accepted, result.stderr)
                    self.assertEqual("evidence accepted" in result.stdout, accepted)

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


@unittest.skipUnless(
    sys.platform == "win32" and shutil.which("pwsh")
    and Path(sys.executable).with_name("pythonw.exe").is_file(),
    "Requires Windows PowerShell and the console-free Python test helper",
)
class GuiRunnerFailureTests(unittest.TestCase):
    """Process-runner failure tests; these do not exercise playback or a GUI."""

    def startup_failure(self, exit_code):
        with tempfile.TemporaryDirectory(prefix="riina-runner-failure-") as directory:
            root = Path(directory)
            helper = root / "startup.py"
            helper.write_text(
                'import sys\nsys.stdout.write("synthetic stdout\\n")\n'
                'sys.stderr.write("synthetic startup failure\\n")\n'
                f'raise SystemExit({exit_code})\n', encoding="utf-8",
            )
            temporary = root / "temporary"
            temporary.mkdir()
            report = root / "report.json"
            runner = Path(__file__).resolve().parents[1] / "smoke-windows-package.ps1"
            result = subprocess.run(
                [shutil.which("pwsh"), "-NoProfile", "-File", str(runner),
                 "-Executable", str(Path(sys.executable).with_name("pythonw.exe")),
                 "-Fixture", str(helper), "-ReportPath", str(report)],
                env={**os.environ, "TEMP": str(temporary), "TMP": str(temporary)},
                capture_output=True, text=True, timeout=25,
                creationflags=subprocess.CREATE_NO_WINDOW,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(report.exists(), "A failed helper must not generate passing GUI evidence")
            self.assertEqual(Path(str(report) + ".stderr.log").read_text(), "synthetic startup failure\n")
            self.assertEqual(Path(str(report) + ".stdout.log").read_text(), "synthetic stdout\n")
            return result.stdout + result.stderr

    def test_nonzero_startup_retains_process_output_and_reports_the_actual_error(self):
        self.assertIn("synthetic startup failure", self.startup_failure(23))

    def test_zero_exit_without_application_report_still_fails_and_retains_output(self):
        self.assertIn("no smoke report", self.startup_failure(0))


if __name__ == "__main__":
    unittest.main()
