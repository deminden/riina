"""Release regressions independent of a native compilation or runner platform."""
import importlib.util
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("release", Path(__file__).parents[1] / "check-release-artifacts.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseArtifactTests(unittest.TestCase):
    def release_archive(self, root, extra=None, manifest_override=None):
        archive = root / "riina-0.3.8-windows-x86_64.zip"
        content = {"libmpv-2.dll": b"engine", "SOURCE-MANIFEST.tsv": b"pinned native sources"}
        manifest = "path\tsha256\n" + "".join(
            f"{name}\t{hashlib.sha256(data).hexdigest()}\n" for name, data in content.items()
        )
        with zipfile.ZipFile(archive, "w") as package:
            for name, data in content.items():
                package.writestr(name, data)
            package.writestr("RUNTIME-MANIFEST.tsv", manifest_override or manifest)
            package.writestr("riina.exe", b"old application must not be restored")
            package.writestr("THIRD_PARTY_LICENSES/RUST/old.txt", b"old application notices")
            if extra:
                # Set the raw archive spelling after ZipInfo's Windows path
                # normalization, so the reader really receives the bad bytes.
                entry = zipfile.ZipInfo("raw-entry")
                entry.filename = extra[0]
                package.writestr(entry, extra[1])
        sums = root / "SHA256SUMS.txt"
        sums.write_text(f"{release.digest(archive)}  {archive.name}\n", encoding="utf-8")
        report = root / "RELEASE-VERIFICATION.json"
        report.write_text(json.dumps({"version": "0.3.8", "source_commit": "a" * 40,
                                     "libmpv_sha256": hashlib.sha256(b"engine").hexdigest()}), encoding="utf-8")
        return archive, sums, report

    @staticmethod
    def native_git_identity(arguments, **_):
        return ("a" * 40 if arguments[-1].endswith("^{commit}") else "b" * 40) + "\n"

    def restore_archive(self, root, inputs):
        with patch.object(release.subprocess, "check_output", side_effect=self.native_git_identity), \
                patch.object(release, "verify_native_source_index") as source_check:
            result = release.restore_release_runtime(*inputs, root, "v0.3.8", root / "restored")
            source_check.assert_called_once()
            return result

    def test_published_runtime_reuse_excludes_old_application_and_app_notices(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            result = self.restore_archive(root, self.release_archive(root))
            self.assertEqual({p.name for p in (root / "restored").iterdir()},
                             {"libmpv-2.dll", "SOURCE-MANIFEST.tsv", "RUNTIME-MANIFEST.tsv"})
            self.assertEqual(result["native_release_tag"], "v0.3.8")
            self.assertEqual(result["libmpv_sha256"], hashlib.sha256(b"engine").hexdigest())

    def test_published_runtime_reuse_rejects_archive_aliases_and_incomplete_inventory(self):
        for name in ("../escaped.dll", "/absolute.dll", "C:/drive.dll", "sub\\file.dll", "LIBMPV-2.DLL"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                inputs = self.release_archive(root, extra=(name, b"unexpected"))
                with self.assertRaisesRegex(ValueError, "archive path"):
                    self.restore_archive(root, inputs)
                self.assertFalse((root / "restored").exists())
                self.assertFalse((root.parent / "escaped.dll").exists())
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            inputs = self.release_archive(root, manifest_override="path\tsha256\nmissing.dll\t" + "0" * 64 + "\n")
            with self.assertRaisesRegex(ValueError, "incomplete"):
                self.restore_archive(root, inputs)
            self.assertFalse((root / "restored").exists())

    def test_published_runtime_reuse_fails_closed_on_checksum_or_build_identity(self):
        for failure in ("checksum", "duplicate checksum", "source commit", "engine hash", "build contract"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                archive, sums, report = inputs = self.release_archive(root)
                if failure == "checksum":
                    sums.write_text("0" * 64 + f"  {archive.name}\n", encoding="utf-8")
                elif failure == "duplicate checksum":
                    sums.write_text(sums.read_text() * 2, encoding="utf-8")
                elif failure in ("source commit", "engine hash"):
                    data = json.loads(report.read_text())
                    data["source_commit" if failure == "source commit" else "libmpv_sha256"] = "0" * 64
                    report.write_text(json.dumps(data), encoding="utf-8")
                def identity(arguments, **kwargs):
                    if failure == "build contract" and arguments[-1] == "HEAD:tools/mpv":
                        return "c" * 40
                    return self.native_git_identity(arguments, **kwargs)
                with patch.object(release.subprocess, "check_output", side_effect=identity), \
                        patch.object(release, "verify_native_source_index"), self.assertRaises(ValueError):
                    release.restore_release_runtime(*inputs, root, "v0.3.8", root / "restored")
                self.assertFalse((root / "restored").exists())
                self.assertEqual(list(root.glob("riina-runtime-*")), [])

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

    def test_native_source_index_records_immutable_sources_and_verified_notices(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            tools = source / "tools/mpv"
            tools.mkdir(parents=True)
            builder_revision = "a" * 40
            curl_revision = "b" * 40
            (tools / "build-windows.sh").write_text(
                f"readonly BUILDER_REVISION='{builder_revision}'\n"
                f"readonly CURL_REVISION='{curl_revision}'\n",
                encoding="utf-8")
            patches = {
                "remove-x264.patch": "x264 exclusion",
                "pin-curl.patch": "curl source pin",
                "native-bootstrap.patch": "native source bootstrap",
            }
            for name, content in patches.items():
                (tools / name).write_text(content, encoding="utf-8")
            binary = root / "binary"
            notices = binary / "THIRD_PARTY_LICENSES/codec"
            notices.mkdir(parents=True)
            license_bytes = b"codec upstream license\n"
            (notices / "LICENSE").write_bytes(license_bytes)
            manifest_rows = [
                f"builder_revision\t{builder_revision}",
                f"curl_revision\t{curl_revision}",
                "native_rust_toolchain\tnightly-2026-10-10",
                "x264_policy\tencoder removed",
                f"builder_patch_sha256\t{release.digest(tools / 'remove-x264.patch')}",
                f"curl_patch_sha256\t{release.digest(tools / 'pin-curl.patch')}",
                f"bootstrap_patch_sha256\t{release.digest(tools / 'native-bootstrap.patch')}",
                "container_image\tghcr.io/example/image@sha256:abc",
                f"source_revision\tsrc_packages/codec\thttps://github.com/example/codec\t{'c' * 40}",
                f"source_sha256\tsrc_packages/codec/LICENSE\t{release.hashlib.sha256(license_bytes).hexdigest()}",
            ]
            (binary / "SOURCE-MANIFEST.tsv").write_text("\n".join(manifest_rows) + "\n", encoding="utf-8")

            self.assertEqual(release.verify_native_source_index(binary, source), (1, 1))

    def test_native_source_index_accepts_collected_subrandr_vendor_notices(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            tools = source / "tools/mpv"
            tools.mkdir(parents=True)
            builder_revision = "a" * 40
            curl_revision = "b" * 40
            (tools / "build-windows.sh").write_text(
                f"readonly BUILDER_REVISION='{builder_revision}'\n"
                f"readonly CURL_REVISION='{curl_revision}'\n",
                encoding="utf-8")
            patches = {
                "remove-x264.patch": "x264 exclusion",
                "pin-curl.patch": "curl source pin",
                "native-bootstrap.patch": "native source bootstrap",
            }
            for name, content in patches.items():
                (tools / name).write_text(content, encoding="utf-8")

            binary = root / "binary"
            license_bytes = b"adler2 license bytes\n"
            license_hash = release.hashlib.sha256(license_bytes).hexdigest()
            inventory = binary / "THIRD_PARTY_LICENSES/NATIVE_RUST/RUST-LICENSES.json"
            inventory.parent.mkdir(parents=True)
            license_path = "packages/adler2-2.0.1/LICENSE-0BSD"
            (binary / "THIRD_PARTY_LICENSES/NATIVE_RUST" / license_path).parent.mkdir(parents=True)
            (binary / "THIRD_PARTY_LICENSES/NATIVE_RUST" / license_path).write_bytes(license_bytes)
            inventory.write_text(json.dumps({"packages": [{
                "name": "adler2",
                "version": "2.0.1",
                "license_files": [{"path": license_path, "sha256": license_hash}],
            }]}), encoding="utf-8")
            android_hash = release.hashlib.sha256(b"android-only license\n").hexdigest()
            manifest_rows = [
                f"builder_revision\t{builder_revision}",
                f"curl_revision\t{curl_revision}",
                "native_rust_toolchain\tnightly-2026-10-10",
                "x264_policy\tencoder removed",
                f"builder_patch_sha256\t{release.digest(tools / 'remove-x264.patch')}",
                f"curl_patch_sha256\t{release.digest(tools / 'pin-curl.patch')}",
                f"bootstrap_patch_sha256\t{release.digest(tools / 'native-bootstrap.patch')}",
                "container_image\tghcr.io/example/image@sha256:abc",
                f"source_revision\tsrc_packages/subrandr\thttps://github.com/example/subrandr\t{'c' * 40}",
                f"source_sha256\tsrc_packages/subrandr/riina-vendor/adler2/LICENSE-0BSD\t{license_hash}",
                f"source_sha256\tsrc_packages/subrandr/riina-vendor/android-activity/LICENSE\t{android_hash}",
            ]
            (binary / "SOURCE-MANIFEST.tsv").write_text("\n".join(manifest_rows) + "\n", encoding="utf-8")

            self.assertEqual(release.verify_native_source_index(binary, source), (2, 1))

    def test_native_source_index_rejects_a_missing_matching_notice(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            tools = source / "tools/mpv"
            tools.mkdir(parents=True)
            (tools / "build-windows.sh").write_text(
                f"readonly BUILDER_REVISION='{'a' * 40}'\nreadonly CURL_REVISION='{'b' * 40}'\n",
                encoding="utf-8")
            for name in ("remove-x264.patch", "pin-curl.patch", "native-bootstrap.patch"):
                (tools / name).write_bytes(b"patch")
            binary = root / "binary"
            binary.mkdir()
            rows = [
                f"builder_revision\t{'a' * 40}", f"curl_revision\t{'b' * 40}",
                "native_rust_toolchain\tnightly-2026-10-10", "x264_policy\tencoder removed",
                f"builder_patch_sha256\t{release.digest(tools / 'remove-x264.patch')}",
                f"curl_patch_sha256\t{release.digest(tools / 'pin-curl.patch')}",
                f"bootstrap_patch_sha256\t{release.digest(tools / 'native-bootstrap.patch')}",
                "container_image\tghcr.io/example/image@sha256:abc",
                f"source_revision\tsrc_packages/codec\thttps://github.com/example/codec\t{'c' * 40}",
                f"source_sha256\tsrc_packages/codec/LICENSE\t{'d' * 64}",
            ]
            (binary / "SOURCE-MANIFEST.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "Packaged native notice differs"):
                release.verify_native_source_index(binary, source)

    def test_native_source_index_accepts_reused_legacy_manifest_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            tools = source / "tools/mpv"
            tools.mkdir(parents=True)
            builder_revision = "a" * 40
            curl_revision = "b" * 40
            (tools / "build-windows.sh").write_text(
                f"readonly BUILDER_REVISION='{builder_revision}'\n"
                f"readonly CURL_REVISION='{curl_revision}'\n",
                encoding="utf-8")
            patches = {
                "remove-x264.patch": "x264 exclusion",
                "pin-curl.patch": "curl source pin",
                "native-bootstrap.patch": "native source bootstrap",
            }
            for name, content in patches.items():
                (tools / name).write_text(content, encoding="utf-8")
            binary = root / "binary"
            notice = binary / "THIRD_PARTY_LICENSES/codec/LICENSE"
            notice.parent.mkdir(parents=True)
            notice.write_bytes(b"legacy notice\n")
            rows = [
                f"builder_revision\t{builder_revision}",
                f"curl_revision\t{curl_revision}",
                "native_rust_toolchain\tnightly-2026-10-10",
                "x264_policy\tencoder removed",
                f"builder_patch_sha256\t{release.digest(tools / 'remove-x264.patch')}",
                f"curl_patch_sha256\t{release.digest(tools / 'pin-curl.patch')}",
                f"bootstrap_patch_sha256\t{release.digest(tools / 'native-bootstrap.patch')}",
                "container_image\tghcr.io/example/image@sha256:abc",
                f"source_git\tsrc_packages/codec\thttps://github.com/example/codec\t{'c' * 40}",
                f"source_sha256\tcodec/LICENSE\t{release.digest(notice)}",
            ]
            (binary / "SOURCE-MANIFEST.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")

            vendor_notices = binary / "THIRD_PARTY_LICENSES/subrandr/riina-vendor/example-crate"
            vendor_notices.mkdir(parents=True)
            (vendor_notices / "LICENSE-MIT").write_text("license only\n", encoding="utf-8")
            self.assertEqual(release.verify_native_source_index(binary, source), (1, 1))

            (vendor_notices / "src.rs").write_text("bundled source\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "duplicated source trees"):
                release.verify_native_source_index(binary, source)

    def test_native_source_index_rejects_source_path_traversal(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            tools = source / "tools/mpv"
            tools.mkdir(parents=True)
            (tools / "build-windows.sh").write_text(
                f"readonly BUILDER_REVISION='{'a' * 40}'\nreadonly CURL_REVISION='{'b' * 40}'\n",
                encoding="utf-8")
            for name in ("remove-x264.patch", "pin-curl.patch", "native-bootstrap.patch"):
                (tools / name).write_bytes(b"patch")
            binary = root / "binary"
            binary.mkdir()
            rows = [
                f"builder_revision\t{'a' * 40}", f"curl_revision\t{'b' * 40}",
                "native_rust_toolchain\tnightly-2026-10-10", "x264_policy\tencoder removed",
                f"builder_patch_sha256\t{release.digest(tools / 'remove-x264.patch')}",
                f"curl_patch_sha256\t{release.digest(tools / 'pin-curl.patch')}",
                f"bootstrap_patch_sha256\t{release.digest(tools / 'native-bootstrap.patch')}",
                "container_image\tghcr.io/example/image@sha256:abc",
                f"source_revision\tsrc_packages/codec\thttps://github.com/example/codec\t{'c' * 40}",
                f"source_sha256\tsrc_packages/codec/../../escape.c\t{'d' * 64}",
            ]
            (binary / "SOURCE-MANIFEST.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "Unsafe native source index path"):
                release.verify_native_source_index(binary, source)


if __name__ == "__main__":
    unittest.main()
