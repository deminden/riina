"""Keep cheap validation on both fresh and reused native artifact paths."""
import unittest

from workflow_reader import action_step, named_step, workflow


class PackageWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.release = workflow("release.yml")["jobs"]
        self.package = self.release["windows-package"]

    def test_exact_candidate_runs_packaging_tests_before_artifact_download(self):
        tests = named_step(self.package, "Packaging regressions for the exact selected source")
        steps = self.package["steps"]
        downloads = [step for step in steps if step.get("uses", "").startswith("actions/download-artifact@")]
        self.assertTrue(downloads)
        for download in downloads:
            self.assertLess(steps.index(tests), steps.index(download))
        self.assertIn("python -m unittest discover", tests["run"])
        self.assertNotIn("if", tests)

    def test_packaging_tests_run_on_pull_requests_without_native_compilation(self):
        checks = workflow("package-checks.yml")
        self.assertIn("pull_request", checks["on"])
        package = checks["jobs"]["packaging"]
        self.assertNotIn("needs", package)
        tests = named_step(package, "Portable package, license and workflow regressions")
        self.assertIn("python -m unittest discover", tests["run"])

    def test_staged_engine_is_verified_before_package_use(self):
        readiness = named_step(self.release["release-readiness"], "Check exact candidate verification status")
        self.assertIn("^[1-9][0-9]*:[a-f0-9]{64}$", readiness["run"])
        self.assertIn("Staged native assets are for private package verification", readiness["run"])
        self.assertIn("outputs.native_asset_id == ''", self.release["native-engine"]["if"])
        self.assertIn("outputs.native_asset_id != ''", self.package["if"])
        restore = named_step(self.package, "Restore verified engine from a staged asset")
        self.assertLess(restore["run"].index("Get-FileHash"), restore["run"].index("Expand-Archive"))
        verify = named_step(self.package, "Verify downloaded native runtime and build identity")
        self.assertLess(self.package["steps"].index(restore), self.package["steps"].index(verify))

    def test_release_checks_the_extracted_application_in_addition_to_engine(self):
        gui = named_step(self.package, "Exercise the extracted application and native controls")
        self.assertIn("smoke-windows-package.ps1 -Executable (Resolve-Path package-smoke/riina.exe).Path", gui["run"])
        build = named_step(self.package, "Build and assemble portable package")
        self.assertIn("playback_pause_seek_stop_failure_and_shutdown", build["run"])
        self.assertIn("--ignored --exact", build["run"])

    def test_release_tests_the_selected_source_without_rewriting_it(self):
        tests = named_step(self.package, "Lint and unit tests")["run"]
        self.assertEqual(tests.strip().splitlines(), [
            "$PSNativeCommandUseErrorActionPreference = $true",
            "cargo clippy --workspace --all-targets --locked -- -D warnings",
            "cargo test --workspace --locked",
        ])

    def test_failed_gui_candidates_retain_output_without_enabling_publication(self):
        evidence = named_step(self.package, "Preserve application smoke evidence")
        self.assertEqual(evidence["if"], "always()")
        for name in ("riina-gui-smoke.json.stderr.log", "riina-gui-smoke.json.stdout.log"):
            self.assertIn(name, evidence["with"]["path"])
        archive = action_step(self.package, "actions/upload-artifact", name="windows-package")
        self.assertEqual(archive["if"], "always() && steps.package-build.outcome == 'success'")
        verification = self.release["package-verification"]
        self.assertIn("windows-package", verification["needs"])
        self.assertIn("needs.windows-package.result == 'success'", verification["if"])
        publication = self.release["publish"]
        self.assertIn("package-verification", publication["needs"])
        self.assertIn("needs.package-verification.result == 'success'", publication["if"])
        self.assertIn("inputs.publish_release", publication["if"])

    def test_successful_compiler_outputs_survive_later_packaging_failure(self):
        restore = named_step(self.package, "Restore Rust dependency and compiler outputs")
        self.assertEqual(restore["id"], "rust-cache")
        self.assertTrue(restore["uses"].startswith("actions/cache/restore@"))
        build = named_step(self.package, "Compile Windows application for the release")
        self.assertEqual(build["id"], "release-build")
        save = named_step(self.package, "Preserve successful compiler outputs")
        self.assertIn("always() && steps.release-build.outcome == 'success'", save["if"])
        self.assertTrue(save["uses"].startswith("actions/cache/save@"))
        self.assertEqual(save["with"]["key"], "${{ steps.rust-cache.outputs.cache-primary-key }}")

    def test_installer_is_checked_before_package_publication(self):
        smoke = named_step(self.package, "Verify installed application, reinstall and uninstall")
        self.assertIn("smoke-windows-installer.ps1", smoke["run"])
        verification = self.release["package-verification"]
        self.assertIn("windows-package", verification["needs"])
        audit = named_step(verification, "Audit exact extracted binary and corresponding source")
        self.assertIn("scripts/check-windows-installer.py", audit["run"])
        publish = named_step(self.release["publish"], "Publish release with binary and source access information")
        for name in ("INSTALLER-VERIFICATION.json", "INSTALLER-SMOKE.json", "windows-x86_64-setup.exe"):
            self.assertIn(name, publish["run"])


if __name__ == "__main__":
    unittest.main()
