"""Audit installer inventory and installed lifecycle evidence against the ZIP."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile


def verify(archive, installer, inventory, smoke):
    record = json.loads(inventory.read_text(encoding="utf-8-sig"))
    lifecycle = json.loads(smoke.read_text(encoding="utf-8-sig"))
    with installer.open("rb") as stream:
        checksum = hashlib.file_digest(stream, "sha256").hexdigest()
    for field, expected in (("version", record["version"]),
                            ("source_commit", record["source_commit"]),
                            ("installer_sha256", checksum)):
        if record[field] != expected or lifecycle.get(field) != expected:
            raise ValueError(f"Installer lifecycle identity differs: {field}")
    if installer.name != record["installer"] or installer.name != f'riina-{record["version"]}-windows-x86_64-setup.exe':
        raise ValueError("Installer filename differs")
    for field in ("installed_payload_matches_portable", "same_version_reinstall",
                  "start_menu_shortcut", "desktop_shortcut_opt_in", "uninstall_registration", "installed_application_smoke",
                  "uninstall", "profile_preserved"):
        if lifecycle.get(field) is not True:
            raise ValueError(f"Missing installed lifecycle check: {field}")
    with zipfile.ZipFile(archive) as package:
        files = [entry for entry in package.infolist() if not entry.is_dir()]
        names = [entry.filename for entry in files]
        if len(names) != len(set(names)) or set(names) != set(record["payload_files"]):
            raise ValueError("Installer inventory differs from portable ZIP")
        for entry in files:
            with package.open(entry) as stream:
                if hashlib.file_digest(stream, "sha256").hexdigest() != record["payload_files"][entry.filename]:
                    raise ValueError(f"Installer payload differs: {entry.filename}")
        if package.read("SOURCE-COMMIT.txt").decode("utf-8-sig").strip() != record["source_commit"]:
            raise ValueError("Installer source differs from portable ZIP")
    return {"installer_sha256": checksum, "payload_files": len(names),
            "source_commit": record["source_commit"], "installed_lifecycle_verified": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ("archive", "installer", "inventory", "smoke"):
        parser.add_argument("--" + argument, type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.archive, args.installer, args.inventory, args.smoke), indent=2))


if __name__ == "__main__":
    main()
