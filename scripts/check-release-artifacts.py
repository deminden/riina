"""Validate release inputs before runners load or package downloaded artifacts."""
import argparse
import csv
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tomllib


def readiness(audit):
    ready = audit["release_artifacts_verified"]
    if type(ready) is not bool:
        raise ValueError("release_artifacts_verified must be a JSON boolean")
    candidate_id = audit.get("verified_candidate_run_id")
    native_id = audit.get("verified_native_run_id")
    for name, value in (("verified_candidate_run_id", candidate_id), ("verified_native_run_id", native_id)):
        if value is not None and (type(value) is not int or value <= 0):
            raise ValueError(f"{name} must be a positive integer or null")
    if ready and (candidate_id is None or native_id is None):
        raise ValueError("Verified release needs separate package and native artifact run IDs")
    # A candidate that reuses an engine does not upload that engine again. Its
    # verification run and the run supplying matching DLL/source artifacts differ.
    return f"release_ready={str(ready).lower()}\nnative_run_id={native_id if ready else ''}\n"


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


def verify_runtime(directory, application=False):
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
    if (not application and recorded != actual) or not {"libmpv-2.dll", "SOURCE-MANIFEST.tsv"} <= recorded:
        raise ValueError("Runtime manifest does not match the complete engine artifact")


def source_path(root, name):
    relative = PurePosixPath(name)
    if (relative.is_absolute() or not name or ".." in relative.parts
            or "\\" in name or ":" in name or str(relative) != name):
        raise ValueError(f"Unsafe source path: {name!r}")
    path = root.joinpath(*relative.parts)
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError(f"Source path escapes archive: {name!r}")
    return path


def verify_native_sources(native, runtime):
    manifest = native / "SOURCE-MANIFEST.tsv"
    if manifest.read_bytes() != (runtime / manifest.name).read_bytes():
        raise ValueError("Native sources do not belong to the packaged engine")
    files, repositories = set(), set()
    for fields in csv.reader(manifest.read_text(encoding="utf-8").splitlines(), delimiter="\t"):
        if fields[0] == "source_sha256":
            if len(fields) != 3 or fields[1] in files:
                raise ValueError("Invalid or duplicate native source hash record")
            path = source_path(native / "native-sources", fields[1])
            if not path.is_file() or digest(path) != fields[2]:
                raise ValueError(f"Native source hash mismatch: {fields[1]}")
            files.add(fields[1])
        elif fields[0] == "source_git":
            if len(fields) != 4 or not fields[1].startswith("src_packages/"):
                raise ValueError("Invalid native repository record")
            path = source_path(native / "native-sources", fields[1].removeprefix("src_packages/"))
            revision = subprocess.check_output(
                ["git", "-c", f"safe.directory={path.resolve()}", "-C", str(path), "rev-parse", "HEAD"],
                text=True).strip()
            if revision != fields[3] or fields[1] in repositories:
                raise ValueError(f"Native repository revision mismatch: {fields[1]}")
            repositories.add(fields[1])
        elif fields[0] in {"builder_patch_sha256", "curl_patch_sha256", "bootstrap_patch_sha256"}:
            name = {"builder_patch_sha256": "remove-x264.patch", "curl_patch_sha256": "pin-curl.patch",
                    "bootstrap_patch_sha256": "native-bootstrap.patch"}[fields[0]]
            if len(fields) != 2 or digest(native / name) != fields[1]:
                raise ValueError(f"Native build patch mismatch: {name}")
    if not files or not repositories or (native / "native-sources/x264").exists():
        raise ValueError("Incomplete native sources or excluded x264 source present")
    for notice in (runtime / "THIRD_PARTY_LICENSES").rglob("*"):
        if not notice.is_file():
            continue
        relative = notice.relative_to(runtime / "THIRD_PARTY_LICENSES")
        if relative.parts[0] in {"RUST", "SLINT-ROYALTY-FREE-2.0.pdf"}:
            continue
        # Native Rust notices are generated alongside source capture; the other
        # notices are copied directly from those captured native checkouts.
        root = native / ("THIRD_PARTY_LICENSES" if relative.parts[0] == "NATIVE_RUST" else "native-sources")
        corresponding = root / relative
        if not corresponding.is_file() or digest(corresponding) != digest(notice):
            raise ValueError(f"Packaged native notice differs from source: {relative}")
    return len(files), len(repositories)


def verify_package(binary, source, version):
    verify_runtime(binary, application=True)
    actual_version = tomllib.loads((source / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
    if actual_version != version:
        raise ValueError("Release version differs from corresponding source")
    for name in ("riina.exe", "LICENSE", "LICENSE-GPL-3.0.txt", "THIRD_PARTY_NOTICES.md"):
        if not (binary / name).is_file():
            raise ValueError(f"Missing application package file: {name}")
        if name != "riina.exe" and (binary / name).read_text(encoding="utf-8") != (source / name).read_text(encoding="utf-8"):
            raise ValueError(f"Application notice differs from source: {name}")
    if not (source / "cargo-vendor").is_dir() or not (source / ".cargo/config.toml").is_file():
        raise ValueError("Missing vendored application source or offline build configuration")
    files, repositories = verify_native_sources(source / "native-build/native-source", binary)
    for name in ("RUST/RUST-LICENSES.json", "SLINT-ROYALTY-FREE-2.0.pdf"):
        if (binary / "THIRD_PARTY_LICENSES" / name).read_bytes() != (source / "THIRD_PARTY_LICENSES" / name).read_bytes():
            raise ValueError(f"Application license inventory differs from source: {name}")
    binary_notices = binary / "THIRD_PARTY_LICENSES/RUST"
    source_notices = source / "THIRD_PARTY_LICENSES/RUST"
    source_files = {path.relative_to(source_notices) for path in source_notices.rglob("*") if path.is_file()}
    binary_files = {path.relative_to(binary_notices) for path in binary_notices.rglob("*") if path.is_file()}
    if source_files != binary_files or any(
            digest(source_notices / name) != digest(binary_notices / name) for name in source_files):
        raise ValueError("Application notice files differ from corresponding source")
    return {"version": version, "native_source_files_verified": files,
            "native_repositories_verified": repositories, "libmpv_sha256": digest(binary / "libmpv-2.dll"),
            "cargo_lock_sha256": digest(source / "Cargo.lock"),
            "scope": "Exact extracted package, corresponding native sources and third-party notices; playback is a separate required workflow check."}


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
    package = commands.add_parser("package")
    package.add_argument("--binary", type=Path, required=True)
    package.add_argument("--source", type=Path, required=True)
    package.add_argument("--version", required=True)
    args = parser.parse_args()
    if args.command == "readiness":
        print(readiness(json.loads(args.audit.read_text(encoding="utf-8"))), end="")
    elif args.command == "checksum":
        verify_checksum(args.file, args.manifest)
        print("Verified downloaded source archive SHA-256")
    elif args.command == "runtime":
        verify_runtime(args.directory)
        print("Verified complete downloaded native runtime manifest")
    else:
        print(json.dumps(verify_package(args.binary, args.source, args.version), indent=2))


if __name__ == "__main__":
    main()
