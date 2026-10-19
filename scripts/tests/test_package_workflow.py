"""Keep cheap validation on both fresh and reused native artifact paths."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class PackageWorkflowTests(unittest.TestCase):
    def test_exact_candidate_runs_packaging_tests_before_artifact_download(self):
        source = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        package = source.split("\n  windows-package:", 1)[1].split("\n  publish:", 1)[0]
        tests = package.index("python -m unittest discover")
        self.assertLess(tests, package.index("uses: actions/download-artifact@"))
        self.assertNotIn("if:", package[package.rfind("- name:", 0, tests):tests])

    def test_packaging_tests_run_on_pull_requests_without_native_compilation(self):
        source = (ROOT / ".github/workflows/package-checks.yml").read_text(encoding="utf-8")
        self.assertIn("  pull_request:", source)
        self.assertIn("python -m unittest discover", source)
        self.assertNotIn("needs:", source)

    def test_release_checks_the_extracted_application_in_addition_to_engine(self):
        source = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn("smoke-windows-package.ps1 -Executable (Resolve-Path package-smoke/riina.exe).Path", source)
        self.assertIn("playback_pause_seek_stop_failure_and_shutdown", source)

    def test_failed_gui_candidates_retain_output_without_enabling_publication(self):
        source = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        package = source.split("\n  windows-package:", 1)[1].split("\n  package-verification:", 1)[0]
        evidence = package.split("- name: Preserve application smoke evidence", 1)[1]
        self.assertIn("if: always()", evidence)
        for name in ("riina-gui-smoke.json.stderr.log", "riina-gui-smoke.json.stdout.log"):
            self.assertIn(name, evidence)
        archive = evidence.split("name: windows-package", 1)[0]
        self.assertIn("if: always() && steps.package-build.outcome == 'success'", archive)
        verification = source.split("\n  package-verification:", 1)[1].split("\n  publish:", 1)[0]
        self.assertIn("needs.windows-package.result == 'success'", verification)
        publication = source.split("\n  publish:", 1)[1]
        self.assertIn("needs.package-verification.result == 'success'", publication)
        self.assertIn("inputs.publish_release", publication)

    def test_successful_compiler_outputs_survive_later_gui_failure(self):
        source = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        package = source.split("\n  windows-package:", 1)[1].split("\n  package-verification:", 1)[0]
        self.assertIn("id: rust-cache", package)
        self.assertIn("uses: actions/cache/restore@", package)
        save = package.split("- name: Preserve successful compiler outputs", 1)[1]
        self.assertIn("if: always() && steps.package-build.outcome == 'success'", save)
        self.assertIn("uses: actions/cache/save@", save)
        self.assertIn("key: ${{ steps.rust-cache.outputs.cache-primary-key }}", save)


if __name__ == "__main__":
    unittest.main()
