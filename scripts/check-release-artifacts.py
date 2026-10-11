"""Validate release inputs before runners load or package downloaded artifacts."""
import argparse
import csv
import hashlib
import json
from pathlib import Path, PurePosixPath
import re


def readiness(audit):
    ready = audit["release_artifacts_verified"]
    if type(ready) is not bool:
        raise ValueError("release_artifacts_verified must be a JSON boolean")
    run_id = audit.get("verified_candidate_run_id")
    if run_id is not None and (type(run_id) is not int or run_id <= 0):
        raise ValueError("verified_candidate_run_id must be a positive integer or null")
    # JSON null denotes a fresh build, not an artifact run named 'None'.
    return f"release_ready={str(ready).lower()}\nnative_run_id={run_id or ''}\n"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_checksum(path, manifest):
    fields = manifest.read_text(encoding="utf-8").split()
    if not fields or not re.fullmatch(r"[0-9a-fA-F]{64}", fields[0]):
        raise ValueError(f"Invalid SHA-256 manifest: {manifest}")
    # The original producer records its absolute CI path. The caller identifies
    # the downloaded file explicitly instead of trusting that obsolete path.
    if digest(path) != fields[0].lower():
        raise ValueError(f"SHA-256 mismatch: {path}")


def verify_runtime(directory):
    manifest = directory / "RUNTIME-MANIFEST.tsv"
    recorded = set()
    with manifest.open(encoding="utf-8", newline="") as stream:
        rows = csv.DictReader(stream, delimiter="\t")
        if rows.fieldnames != ["path", "sha256"]:
            raise ValueError("Invalid runtime manifest header")
        for row in rows:
            name = row["path"]
            relative = PurePosixPath(name)
            if (not name or relative.is_absolute() or ".." in relative.parts
                    or "\\" in name or ":" in name or str(relative) != name
                    or name == manifest.name or name in recorded):
                raise ValueError(f"Invalid or duplicate runtime path: {name!r}")
            path = directory.joinpath(*relative.parts)
            if path.is_symlink() or not path.is_file():
                raise ValueError(f"Missing runtime file: {name}")
            expected = row["sha256"]
            if not re.fullmatch(r"[0-9a-f]{64}", expected or "") or digest(path) != expected:
                raise ValueError(f"Runtime SHA-256 mismatch: {name}")
            recorded.add(name)
    actual = {path.relative_to(directory).as_posix()
              for path in directory.rglob("*") if path.is_file() and path != manifest}
    if recorded != actual or not {"libmpv-2.dll", "SOURCE-MANIFEST.tsv"} <= recorded:
        raise ValueError("Runtime manifest does not match the complete engine artifact")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    audit = commands.add_parser("readiness")
    audit.add_argument("--audit", type=Path, default=Path("docs/native-distribution-audit.json"))
    checksum = commands.add_parser("checksum")
    checksum.add_argument("--file", type=Path, required=True)
    checksum.add_argument("--manifest", type=Path, required=True)
    runtime = commands.add_parser("runtime")
    runtime.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "readiness":
        print(readiness(json.loads(args.audit.read_text(encoding="utf-8"))), end="")
    elif args.command == "checksum":
        verify_checksum(args.file, args.manifest)
        print("Verified downloaded source archive SHA-256")
    else:
        verify_runtime(args.directory)
        print("Verified complete downloaded native runtime manifest")


if __name__ == "__main__":
    main()
