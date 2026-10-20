"""Installer inventory checks run without Windows or the compiler."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile


ROOT = Path(__file__).resolve().parents[2]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


builder = load("build-windows-installer")
checker = load("check-windows-installer")
REQUIRED = ("riina.exe", "libmpv-2.dll", "LICENSE", "LICENSE-GPL-3.0.txt",
            "SOURCE-COMMIT.txt", "SOURCE-ACCESS.md", "SOURCE-MANIFEST.tsv",
            "RUNTIME-MANIFEST.tsv", "THIRD_PARTY_NOTICES.md")


class InstallerTests(unittest.TestCase):
    def package(self, root):
        package = root / "portable"
        package.mkdir()
        for name in REQUIRED:
            (package / name).write_bytes(b"a" * 40 if name == "SOURCE-COMMIT.txt" else name.encode())
        notice = package / "THIRD_PARTY_LICENSES" / "nested license.txt"
        notice.parent.mkdir()
        notice.write_text("retained notice", encoding="utf-8")
        return package

    def test_complete_payload_includes_nested_notices(self):
        with tempfile.TemporaryDirectory() as temporary:
            package = self.package(Path(temporary))
            files = builder.payload_files(package)
            self.assertEqual(len(files), len(REQUIRED) + 1)
            self.assertEqual(files["THIRD_PARTY_LICENSES/nested license.txt"],
                             hashlib.sha256(b"retained notice").hexdigest())
            (package / "LICENSE").unlink()
            with self.assertRaisesRegex(ValueError, "missing required"):
                builder.payload_files(package)

    def test_rejects_inno_expansion_uninstall_and_hosted_graphics(self):
        for name in ("{tmp}.txt", "unins000.exe", "opengl32.dll", "libgallium_wgl.dll"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                package = self.package(Path(temporary))
                (package / name).write_text("unexpected", encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "Unsafe or test-only"):
                    builder.payload_files(package)

    def inputs(self, root):
        package = self.package(root)
        inventory = builder.payload_files(package)
        archive = root / "riina-4.2.0-windows-x86_64.zip"
        with zipfile.ZipFile(archive, "w") as output:
            for name in inventory:
                output.write(package / name, name)
        installer = root / "riina-4.2.0-windows-x86_64-setup.exe"
        installer.write_bytes(b"installer")
        record = {"version": "4.2.0", "source_commit": "a" * 40,
                  "installer": installer.name, "installer_sha256": builder.digest(installer),
                  "payload_files": inventory}
        lifecycle = {key: record[key] for key in ("version", "source_commit", "installer_sha256")}
        lifecycle.update(dict.fromkeys(("installed_payload_matches_portable", "same_version_reinstall",
                                       "start_menu_shortcut", "desktop_shortcut_opt_in", "uninstall_registration",
                                       "installed_application_smoke", "uninstall", "profile_preserved"), True))
        report, smoke = root / "inventory.json", root / "smoke.json"
        report.write_text(json.dumps(record), encoding="utf-8")
        smoke.write_text(json.dumps(lifecycle), encoding="utf-8")
        return archive, installer, report, smoke

    def test_installer_identity_payload_and_lifecycle_must_agree(self):
        with tempfile.TemporaryDirectory() as temporary:
            inputs = self.inputs(Path(temporary))
            self.assertTrue(checker.verify(*inputs)["installed_lifecycle_verified"])
            inputs[1].write_bytes(b"replaced installer")
            with self.assertRaisesRegex(ValueError, "identity differs"):
                checker.verify(*inputs)

    def test_rejects_missing_notice_changed_payload_and_failed_uninstall(self):
        for change in ("notice", "payload", "uninstall", "source"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                inputs = self.inputs(Path(temporary))
                if change == "uninstall":
                    report = json.loads(inputs[3].read_text())
                    report["uninstall"] = False
                    inputs[3].write_text(json.dumps(report))
                else:
                    report = json.loads(inputs[2].read_text())
                    if change == "notice":
                        del report["payload_files"]["LICENSE"]
                    elif change == "payload":
                        report["payload_files"]["riina.exe"] = "0" * 64
                    else:
                        report["source_commit"] = "b" * 40
                    inputs[2].write_text(json.dumps(report))
                with self.assertRaises(ValueError):
                    checker.verify(*inputs)


if __name__ == "__main__":
    unittest.main()
