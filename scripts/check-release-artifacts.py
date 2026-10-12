"""Validate release inputs before runners load or package downloaded artifacts."""
import argparse
import csv
import hashlib
import json
import os
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
    # Reused engines supply the runtime and source index from an earlier run.
    return f"release_ready={str(ready).lower()}\nnative_run_id={native_id if ready else ''}\n"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


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


def verify_native_source_index(binary, source):
    """Verify the release's pinned source references and packaged notices."""
    manifest = binary / "SOURCE-MANIFEST.tsv"
    records = {}
    source_files = set()
    for fields in csv.reader(manifest.read_text(encoding="utf-8").splitlines(), delimiter="\t"):
        if not fields:
            raise ValueError("Empty native source index record")
        kind = fields[0]
        if kind in {"builder_revision", "curl_revision", "native_rust_toolchain", "x264_policy",
                    "builder_patch_sha256", "curl_patch_sha256", "bootstrap_patch_sha256", "container_image"}:
            if len(fields) != 2 or kind in records:
                raise ValueError(f"Invalid or duplicate native source index field: {kind}")
            records[kind] = fields[1]
        elif kind == "source_revision":
            if (len(fields) != 4 or not fields[1].startswith("src_packages/")
                    or fields[1] in records or not fields[2].startswith("https://")
                    or not re.fullmatch(r"[0-9a-f]{40,64}", fields[3])):
                raise ValueError("Invalid native source repository reference")
            records[fields[1]] = fields[3]
        elif kind == "source_sha256":
            if (len(fields) != 3 or fields[1] in source_files
                    or not re.fullmatch(r"[0-9a-f]{64}", fields[2])):
                raise ValueError("Invalid or duplicate native source file hash")
            relative = PurePosixPath(fields[1])
            if (relative.is_absolute() or ".." in relative.parts or "\\" in fields[1]
                    or ":" in fields[1] or str(relative) != fields[1]
                    or len(relative.parts) < 3 or relative.parts[0] != "src_packages"):
                raise ValueError(f"Unsafe native source index path: {fields[1]!r}")
            source_files.add(fields[1])
            if Path(fields[1]).name.casefold().startswith(("license", "licence", "copying", "notice")):
                notice_relative = relative.parts[1:]
                notice = binary / "THIRD_PARTY_LICENSES" / Path(*notice_relative)
                if not notice.is_file() or digest(notice) != fields[2]:
                    raise ValueError(f"Packaged native notice differs from its pinned source: {fields[1]}")
        elif kind == "source_symlink":
            if len(fields) != 3 or fields[1] in source_files or not fields[2]:
                raise ValueError("Invalid or duplicate native source symlink")
            relative = PurePosixPath(fields[1])
            if (relative.is_absolute() or ".." in relative.parts or "\\" in fields[1]
                    or ":" in fields[1] or str(relative) != fields[1]
                    or len(relative.parts) < 3 or relative.parts[0] != "src_packages"):
                raise ValueError(f"Unsafe native source index symlink: {fields[1]!r}")
            source_files.add(fields[1])
        else:
            raise ValueError(f"Unknown native source index record: {kind}")

    required = {
        "builder_revision", "curl_revision", "native_rust_toolchain", "x264_policy",
        "builder_patch_sha256", "curl_patch_sha256", "bootstrap_patch_sha256", "container_image",
    }
    if not required <= records.keys() or not source_files or not any(
        name.startswith("src_packages/") for name in records
    ):
        raise ValueError("Native source index is incomplete")
    build_script = (source / "tools/mpv/build-windows.sh").read_text(encoding="utf-8")
    for constant, field in (("BUILDER_REVISION", "builder_revision"), ("CURL_REVISION", "curl_revision")):
        match = re.search(rf"readonly {constant}='([0-9a-f]+)'", build_script)
        if not match or match.group(1) != records[field]:
            raise ValueError(f"Native source index does not match {constant}")
    for field, filename in (("builder_patch_sha256", "remove-x264.patch"),
                            ("curl_patch_sha256", "pin-curl.patch"),
                            ("bootstrap_patch_sha256", "native-bootstrap.patch")):
        if digest(source / "tools/mpv" / filename) != records[field]:
            raise ValueError(f"Native source index patch hash does not match {filename}")
    duplicated_sources = {"cargo-vendor", "riina-vendor", "native-sources"}
    if any(path.name in duplicated_sources for path in binary.rglob("*")):
        raise ValueError("Release binary unexpectedly contains duplicated source trees")
    return len(source_files), sum(name.startswith("src_packages/") for name in records)


def verify_package(binary, source, version):
    verify_runtime(binary, application=True)
    actual_version = tomllib.loads((source / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
    if actual_version != version:
        raise ValueError("Release version differs from corresponding source")
    source_commit = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if (binary / "SOURCE-COMMIT.txt").read_text(encoding="utf-8").strip() != source_commit:
        raise ValueError("Release binary does not identify the checked-out application source commit")
    if ((binary / "SOURCE-ACCESS.md").read_bytes()
            != (source / "docs/SOURCE-ACCESS.md").read_bytes()):
        raise ValueError("Release source-access instructions differ from the tagged source")
    for name in ("riina.exe", "LICENSE", "LICENSE-GPL-3.0.txt", "THIRD_PARTY_NOTICES.md"):
        if not (binary / name).is_file():
            raise ValueError(f"Missing application package file: {name}")
        if name != "riina.exe" and (binary / name).read_text(encoding="utf-8") != (source / name).read_text(encoding="utf-8"):
            raise ValueError(f"Application notice differs from source: {name}")
    files, repositories = verify_native_source_index(binary, source)
    binary_notices = binary / "THIRD_PARTY_LICENSES/RUST"
    rust_inventory = json.loads((binary_notices / "RUST-LICENSES.json").read_text(encoding="utf-8"))
    webpki_roots = [
        package for package in rust_inventory["packages"]
        if package.get("name") == "webpki-roots" and package.get("version") == "1.0.9"
    ]
    if len(webpki_roots) != 1 or webpki_roots[0].get("selected_license") != "CDLA-Permissive-2.0":
        raise ValueError("Rust license inventory is missing the reviewed webpki-roots data grant")
    exception = webpki_roots[0].get("reviewed_data_license_exception", {})
    attribution_relative = PurePosixPath(exception.get("required_attribution_file", ""))
    if attribution_relative.is_absolute() or ".." in attribution_relative.parts:
        raise ValueError("Invalid webpki-roots attribution path")
    attribution_file = binary_notices.joinpath(*attribution_relative.parts)
    if not attribution_file.is_file() or "Common CA Database (CCADB)" not in attribution_file.read_text(encoding="utf-8"):
        raise ValueError("Application package is missing the webpki-roots CCADB attribution")
    if not any(
        item.get("path", "").startswith("packages/webpki-roots-1.0.9/")
        and Path(item["path"]).name.casefold() == "license"
        for item in webpki_roots[0].get("license_files", [])
    ):
        raise ValueError("Rust license inventory is missing the original webpki-roots LICENSE")
    return {"version": version, "native_source_files_indexed": files,
            "native_source_repositories_indexed": repositories, "libmpv_sha256": digest(binary / "libmpv-2.dll"),
            "cargo_lock_sha256": digest(source / "Cargo.lock"),
            "source_commit": source_commit,
            "scope": "Exact extracted package and tagged application source; native source repositories and revisions are recorded in the package manifest; third-party notices are verified; playback is a separate required workflow check."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    audit = commands.add_parser("readiness")
    audit.add_argument("--audit", type=Path, default=Path("docs/native-distribution-audit.json"))
    runtime = commands.add_parser("runtime")
    runtime.add_argument("--directory", type=Path, required=True)
    package = commands.add_parser("package")
    package.add_argument("--binary", type=Path, required=True)
    package.add_argument("--source", type=Path, required=True)
    package.add_argument("--version", required=True)
    args = parser.parse_args()
    if args.command == "readiness":
        print(readiness(json.loads(args.audit.read_text(encoding="utf-8"))), end="")
    elif args.command == "runtime":
        verify_runtime(args.directory)
        print("Verified complete downloaded native runtime manifest")
    else:
        print(json.dumps(verify_package(args.binary, args.source, args.version), indent=2))


if __name__ == "__main__":
    main()
