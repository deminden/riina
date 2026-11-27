"""Release regressions independent of a native compilation or runner platform."""
import importlib.util
import hashlib
import json
from pathlib import Path
import re
import subprocess
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

    def test_native_image_follows_the_selected_commit_after_a_move_to_newer_source(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            def git(*args):
                return subprocess.check_output(
                    ["git", "-C", str(root), "-c", "user.name=Fixture",
                     "-c", "user.email=fixture@example.test", *args],
                    text=True, stderr=subprocess.STDOUT,
                ).strip()

            git("init", "--quiet")
            recipe = root / "tools/mpv/build-windows.sh"
            recipe.parent.mkdir(parents=True)
            base = "ghcr.io/shinchiro/archlinux@sha256:"
            images = [base + "a" * 64, base + "b" * 64, "ghcr.io/shinchiro/archlinux:latest"]
            commits = []
            for image in images:
                recipe.write_text(f"readonly BUILD_IMAGE='{image}'\n", encoding="utf-8")
                git("add", "tools/mpv/build-windows.sh")
                git("commit", "--quiet", "-m", "Record fixture image")
                commits.append(git("rev-parse", "HEAD"))

            self.assertEqual(release.build_image(root, commits[0]), images[0])
            self.assertEqual(release.build_image(root, commits[1]), images[1])
            with self.assertRaisesRegex(ValueError, "pin its build image"):
                release.build_image(root, commits[2])
            with self.assertRaisesRegex(ValueError, "full commit ID"):
                release.build_image(root, "HEAD")

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

    def native_contract(self, source):
        tools = source / "tools/mpv"
        tools.mkdir(parents=True)
        builder_revision = "a" * 40
        curl_revision = "b" * 40
        mpv_revision = "e" * 40
        build_image = "ghcr.io/example/image@sha256:" + "f" * 64
        (tools / "build-windows.sh").write_text(
            f"readonly BUILDER_REVISION='{builder_revision}'\n"
            f"readonly CURL_REVISION='{curl_revision}'\n"
            f"readonly MPV_REVISION='{mpv_revision}'\n"
            f"readonly BUILD_IMAGE='{build_image}'\n",
            encoding="utf-8")
        patches = {
            "builder_patch_sha256": "remove-x264.patch",
            "curl_patch_sha256": "pin-curl.patch",
            "bootstrap_patch_sha256": "native-bootstrap.patch",
            "field_order_patch_sha256": "matroska-field-order.patch",
        }
        for name in patches.values():
            (tools / name).write_text(name, encoding="utf-8")
        return [
            f"builder_revision\t{builder_revision}",
            f"curl_revision\t{curl_revision}",
            "native_rust_toolchain\tnightly-2026-10-10",
            "x264_policy\tencoder removed",
            *(f"{field}\t{release.digest(tools / name)}" for field, name in patches.items()),
            f"build_script_sha256\t{release.digest(tools / 'build-windows.sh')}",
            f"container_image\t{build_image}",
            f"source_revision\tsrc_packages/mpv\thttps://github.com/mpv-player/mpv.git\t{mpv_revision}",
        ]

    def test_native_source_index_records_immutable_sources_and_verified_notices(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            contract = self.native_contract(source)
            binary = root / "binary"
            notices = binary / "THIRD_PARTY_LICENSES/codec"
            notices.mkdir(parents=True)
            license_bytes = b"codec upstream license\n"
            (notices / "LICENSE").write_bytes(license_bytes)
            manifest_rows = [*contract,
                f"source_revision\tsrc_packages/codec\thttps://github.com/example/codec\t{'c' * 40}",
                f"source_sha256\tsrc_packages/codec/LICENSE\t{release.hashlib.sha256(license_bytes).hexdigest()}",
            ]
            (binary / "SOURCE-MANIFEST.tsv").write_text("\n".join(manifest_rows) + "\n", encoding="utf-8")

            self.assertEqual(release.verify_native_source_index(binary, source), (1, 2))

            manifest = binary / "SOURCE-MANIFEST.tsv"
            for field, expected in [
                ("field_order_patch_sha256", "matroska-field-order.patch"),
                ("build_script_sha256", "build-windows.sh"),
                ("container_image", "BUILD_IMAGE"),
            ]:
                changed = [
                    f"{field}\t{'0' * 64}" if row.startswith(field + "\t") else row
                    for row in manifest_rows
                ]
                manifest.write_text("\n".join(changed) + "\n", encoding="utf-8")
                with self.assertRaisesRegex(ValueError, re.escape(expected)):
                    release.verify_native_source_index(binary, source)
            stripped = [row for row in manifest_rows if not row.startswith("field_order_patch_sha256\t")]
            manifest.write_text("\n".join(stripped) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "Native source index is incomplete"):
                release.verify_native_source_index(binary, source)
            wrong_mpv = [
                row[:-40] + "0" * 40 if row.startswith("source_revision\tsrc_packages/mpv\t") else row
                for row in manifest_rows
            ]
            manifest.write_text("\n".join(wrong_mpv) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "MPV_REVISION"):
                release.verify_native_source_index(binary, source)


    def test_native_source_index_accepts_collected_subrandr_vendor_notices(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            contract = self.native_contract(source)
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
            manifest_rows = [*contract,
                f"source_revision\tsrc_packages/subrandr\thttps://github.com/example/subrandr\t{'c' * 40}",
                f"source_sha256\tsrc_packages/subrandr/riina-vendor/adler2/LICENSE-0BSD\t{license_hash}",
                f"source_sha256\tsrc_packages/subrandr/riina-vendor/android-activity/LICENSE\t{android_hash}",
            ]
            (binary / "SOURCE-MANIFEST.tsv").write_text("\n".join(manifest_rows) + "\n", encoding="utf-8")

            self.assertEqual(release.verify_native_source_index(binary, source), (2, 2))

    def test_native_source_index_rejects_a_missing_matching_notice(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            contract = self.native_contract(source)
            binary = root / "binary"
            binary.mkdir()
            rows = [*contract,
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
            contract = self.native_contract(source)
            binary = root / "binary"
            notice = binary / "THIRD_PARTY_LICENSES/codec/LICENSE"
            notice.parent.mkdir(parents=True)
            notice.write_bytes(b"legacy notice\n")
            rows = [*contract,
                f"source_git\tsrc_packages/codec\thttps://github.com/example/codec\t{'c' * 40}",
                f"source_sha256\tcodec/LICENSE\t{release.digest(notice)}",
            ]
            (binary / "SOURCE-MANIFEST.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")

            vendor_notices = binary / "THIRD_PARTY_LICENSES/subrandr/riina-vendor/example-crate"
            vendor_notices.mkdir(parents=True)
            (vendor_notices / "LICENSE-MIT").write_text("license only\n", encoding="utf-8")
            self.assertEqual(release.verify_native_source_index(binary, source), (1, 2))

            (vendor_notices / "src.rs").write_text("bundled source\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "duplicated source trees"):
                release.verify_native_source_index(binary, source)

    def test_native_source_index_rejects_source_path_traversal(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "source"
            contract = self.native_contract(source)
            binary = root / "binary"
            binary.mkdir()
            rows = [*contract,
                f"source_revision\tsrc_packages/codec\thttps://github.com/example/codec\t{'c' * 40}",
                f"source_sha256\tsrc_packages/codec/../../escape.c\t{'d' * 64}",
            ]
            (binary / "SOURCE-MANIFEST.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "Unsafe native source index path"):
                release.verify_native_source_index(binary, source)


if __name__ == "__main__":
    unittest.main()
