"""Release regressions independent of a native compilation or runner platform."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("release", Path(__file__).parents[1] / "check-release-artifacts.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseArtifactTests(unittest.TestCase):
    def test_unverified_release_never_selects_artifacts_and_verified_release_uses_engine_run(self):
        self.assertEqual(release.readiness({"release_artifacts_verified": False, "verified_candidate_run_id": None}),
                         "release_ready=false\nnative_run_id=\n")
        self.assertEqual(release.readiness({"release_artifacts_verified": True, "verified_candidate_run_id": 123,
                                            "verified_native_run_id": 456}),
                         "release_ready=true\nnative_run_id=456\n")
        for missing in ("verified_candidate_run_id", "verified_native_run_id"):
            audit = {"release_artifacts_verified": True, "verified_candidate_run_id": 123,
                     "verified_native_run_id": 456}
            del audit[missing]
            with self.subTest(missing=missing), self.assertRaises(ValueError):
                release.readiness(audit)

    def test_invalid_readiness_types_cannot_select_release_or_artifact_download(self):
        with self.assertRaises(ValueError):
            release.readiness({"release_artifacts_verified": "false"})
        for run_id in (False, "None", "123", 0, -1):
            with self.subTest(run_id=run_id), self.assertRaises(ValueError):
                release.readiness({"release_artifacts_verified": True, "verified_candidate_run_id": run_id})

    def test_downloaded_source_checksum_accepts_producer_path_but_rejects_corruption(self):
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / "source.tar.zst"
            source.write_bytes(b"corresponding source")
            manifest = Path(temp) / "checksum"
            manifest.write_text(f"{release.digest(source)}  /old/runner/source.tar.zst\n", encoding="utf-8")
            release.verify_checksum(source, manifest)
            source.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "mismatch"):
                release.verify_checksum(source, manifest)

    def test_runtime_manifest_rejects_corruption_omission_and_unsafe_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            engine = root / "libmpv-2.dll"
            sources = root / "SOURCE-MANIFEST.tsv"
            engine.write_bytes(b"engine")
            sources.write_bytes(b"source revisions")
            entries = f"libmpv-2.dll\t{release.digest(engine)}\nSOURCE-MANIFEST.tsv\t{release.digest(sources)}\n"
            manifest = root / "RUNTIME-MANIFEST.tsv"
            manifest.write_text("path\tsha256\n" + entries, encoding="utf-8")
            release.verify_runtime(root)
            engine.write_bytes(b"modified")
            with self.assertRaisesRegex(ValueError, "mismatch"):
                release.verify_runtime(root)
            engine.write_bytes(b"engine")
            sources.unlink()
            with self.assertRaisesRegex(ValueError, "Missing"):
                release.verify_runtime(root)
            sources.write_bytes(b"source revisions")
            (root / "extra.dll").write_bytes(b"unrecorded")
            with self.assertRaisesRegex(ValueError, "complete"):
                release.verify_runtime(root)
            (root / "extra.dll").unlink()
            manifest.write_text("path\tsha256\n" + entries + entries, encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "duplicate"):
                release.verify_runtime(root)
            manifest.write_text("path\tsha256\n../outside.dll\t" + "0" * 64 + "\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "path"):
                release.verify_runtime(root)

    def test_sources_must_match_engine_revision_bytes_and_notices(self):
        with tempfile.TemporaryDirectory() as temp:
            native = Path(temp) / "native"
            runtime = Path(temp) / "runtime"
            source = native / "native-sources/codec"
            notices = runtime / "THIRD_PARTY_LICENSES/codec"
            source.mkdir(parents=True)
            notices.mkdir(parents=True)
            (source / "decode.c").write_bytes(b"original source")
            (source / "LICENSE").write_bytes(b"original notice")
            (notices / "LICENSE").write_bytes(b"original notice")
            manifest = ("source_git\tsrc_packages/codec\thttps://example.org/codec\trevision\n"
                        f"source_sha256\tcodec/decode.c\t{release.digest(source / 'decode.c')}\n")
            (native / "SOURCE-MANIFEST.tsv").write_text(manifest, encoding="utf-8")
            (runtime / "SOURCE-MANIFEST.tsv").write_text(manifest, encoding="utf-8")
            with patch.object(release.subprocess, "check_output", return_value="revision\n"):
                self.assertEqual(release.verify_native_sources(native, runtime), (1, 1))
                (source / "decode.c").write_bytes(b"different source")
                with self.assertRaisesRegex(ValueError, "hash mismatch"):
                    release.verify_native_sources(native, runtime)
                (source / "decode.c").write_bytes(b"original source")
                (notices / "LICENSE").write_bytes(b"different notice")
                with self.assertRaisesRegex(ValueError, "notice differs"):
                    release.verify_native_sources(native, runtime)
                (notices / "LICENSE").write_bytes(b"original notice")
                (runtime / "SOURCE-MANIFEST.tsv").write_text("different engine", encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "do not belong"):
                    release.verify_native_sources(native, runtime)

    def test_source_paths_reject_traversal_and_absolute_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in ("../outside", "/outside", "C:/outside", "codec\\outside"):
                with self.subTest(name=name), self.assertRaises(ValueError):
                    release.source_path(root, name)


if __name__ == "__main__":
    unittest.main()
