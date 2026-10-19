"""Reuse a built binary without accepting failed build gates or foreign artifacts."""
import copy
import hashlib
import importlib.util
import io
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest
from unittest.mock import patch


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).parents[1] / filename)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


candidate = module("candidate", "select-windows-candidate.py")
graphics = module("graphics", "prepare-windows-ci-graphics.py")


class SavedCandidateTests(unittest.TestCase):
    def inputs(self):
        return [
            {"id": 42, "status": "completed", "event": "workflow_dispatch",
             "path": ".github/workflows/release.yml", "head_sha": "a" * 40,
             "repository": {"full_name": "owner/riina"},
             "head_repository": {"full_name": "owner/riina"}},
            [{"name": "windows-package", "conclusion": "failure", "steps": [
                {"name": name, "conclusion": "success"} for name in candidate.REQUIRED_STEPS
            ] + [{"name": "Exercise the extracted application and native controls", "conclusion": "failure"}]}],
            [{"id": 7, "name": "windows-package", "expired": False,
              "workflow_run": {"id": 42, "head_sha": "a" * 40}}],
        ]

    def select(self, inputs):
        return candidate.select(*inputs, "owner/riina", 42)

    def test_successful_build_can_be_retested_after_gui_failure(self):
        self.assertEqual(self.select(self.inputs()), {
            "source_sha": "a" * 40, "artifact_id": 7, "candidate_run_id": 42})

    def test_every_required_original_build_gate_must_succeed(self):
        for name in candidate.REQUIRED_STEPS:
            for conclusion in ("failure", "skipped", None):
                with self.subTest(step=name, conclusion=conclusion):
                    inputs = self.inputs()
                    next(s for s in inputs[1][0]["steps"] if s["name"] == name)["conclusion"] = conclusion
                    with self.assertRaises(ValueError):
                        self.select(inputs)

    def test_missing_or_duplicate_job_step_and_artifact_are_rejected(self):
        for index in (1, 2):
            for duplicate in (False, True):
                inputs = self.inputs()
                inputs[index] = inputs[index] * 2 if duplicate else []
                with self.subTest(index=index, duplicate=duplicate), self.assertRaises(ValueError):
                    self.select(inputs)
        for duplicate in (False, True):
            inputs = self.inputs()
            steps = inputs[1][0]["steps"]
            steps.append(copy.deepcopy(steps[0])) if duplicate else steps.pop(0)
            with self.assertRaises(ValueError):
                self.select(inputs)

    def test_other_repository_workflow_revision_or_incomplete_run_is_rejected(self):
        for field, value in (("id", 99), ("status", "in_progress"), ("event", "pull_request"),
                             ("path", ".github/workflows/other.yml"), ("head_sha", "main"),
                             ("repository", {"full_name": "foreign/riina"}),
                             ("head_repository", {"full_name": "foreign/riina"})):
            inputs = self.inputs()
            inputs[0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.select(inputs)

    def test_expired_and_mismatched_artifacts_are_rejected(self):
        for field, value in (("id", True), ("id", 0), ("expired", True), ("expired", "false"),
                             ("workflow_run", {"id": 99, "head_sha": "a" * 40}),
                             ("workflow_run", {"id": 42, "head_sha": "b" * 40})):
            inputs = self.inputs()
            inputs[2][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.select(inputs)


@unittest.skipUnless(shutil.which("tar"), "Requires the CI archive extractor")
class CiGraphicsTests(unittest.TestCase):
    def test_only_reviewed_dlls_are_staged_and_application_is_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            app = root / "app"
            app.mkdir()
            (app / "riina.exe").write_bytes(b"unchanged application")
            archive = root / "mesa.tar"
            with tarfile.open(archive, "w") as output:
                for name in (*graphics.FILES, "unrelated.dll"):
                    data = name.encode()
                    item = tarfile.TarInfo("x64/" + name)
                    item.size = len(data)
                    output.addfile(item, io.BytesIO(data))
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            with self.assertRaisesRegex(ValueError, "checksum"):
                graphics.prepare(archive, app)
            self.assertEqual(list(app.iterdir()), [app / "riina.exe"])
            with patch.object(graphics, "ARCHIVE_SHA256", digest):
                graphics.prepare(archive, app)
                with self.assertRaisesRegex(ValueError, "replace"):
                    graphics.prepare(archive, app)
            self.assertEqual((app / "riina.exe").read_bytes(), b"unchanged application")
            self.assertEqual({p.name for p in app.iterdir()}, {"riina.exe", *graphics.FILES})
            for name in graphics.FILES:
                self.assertEqual((app / name).read_bytes(), name.encode())


class ReverificationWorkflowTests(unittest.TestCase):
    def test_reverification_has_no_build_and_publishes_only_after_smoke(self):
        root = Path(__file__).resolve().parents[2]
        source = (root / ".github/workflows/verify-windows-package.yml").read_text()
        self.assertNotIn("cargo build", source)
        self.assertNotIn("build-windows.sh", source)
        self.assertIn("steps.candidate.outputs.source_sha", source)
        self.assertIn("steps.candidate.outputs.artifact_id", source)
        self.assertIn("--source candidate-source", source)
        self.assertLess(source.index("check-release-artifacts.py package"), source.index("prepare-windows-ci-graphics.py"))
        self.assertLess(source.index("smoke-windows-package.ps1"), source.index("name: verified-release-assets"))
        self.assertNotIn("if: always()", source.split("- name: Preserve verified original release assets", 1)[1])
        self.assertNotIn("release create", source)


if __name__ == "__main__":
    unittest.main()
