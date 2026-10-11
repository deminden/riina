"""Normalize native notices to the exact captured sources before release audit."""
import argparse
import csv
import hashlib
from pathlib import Path, PurePosixPath
import zipfile


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def license_name(path):
    name = path.name.casefold()
    return name.startswith(("license", "licence", "copying", "notice"))


def prepare(binary, native_source):
    """Use only notices present in the archived source state; preserve binaries."""
    manifest = native_source / "SOURCE-MANIFEST.tsv"
    rows = csv.reader(manifest.read_text(encoding="utf-8").splitlines(), delimiter="\t")
    notices = {}
    for fields in rows:
        if not fields or fields[0] != "source_sha256":
            continue
        if len(fields) != 3:
            raise ValueError("Invalid native source hash record")
        relative = PurePosixPath(fields[1])
        if (relative.is_absolute() or not relative.parts or ".." in relative.parts
                or "\\" in fields[1] or ":" in fields[1] or str(relative) != fields[1]):
            raise ValueError(f"Unsafe native source path: {fields[1]!r}")
        source_file = native_source / "native-sources" / Path(*relative.parts)
        if not source_file.is_file() or sha256(source_file) != fields[2]:
            raise ValueError(f"Native source hash mismatch: {fields[1]}")
        if license_name(source_file):
            notices[relative.as_posix()] = source_file
    if not notices:
        raise ValueError("The native source manifest contains no license notices")

    runtime = binary / "THIRD_PARTY_LICENSES"
    if not runtime.is_dir():
        raise ValueError("The portable package has no native license directory")
    runtime_manifest = binary / "RUNTIME-MANIFEST.tsv"
    with runtime_manifest.open(encoding="utf-8", newline="") as stream:
        rows = csv.reader(stream, delimiter="\t")
        if next(rows, None) != ["path", "sha256"]:
            raise ValueError("Invalid native runtime manifest header")
        entries = {}
        for fields in rows:
            if len(fields) != 2:
                raise ValueError("Invalid native runtime manifest record")
            name, expected = fields
            relative = PurePosixPath(name)
            if (not name or relative.is_absolute() or not relative.parts or ".." in relative.parts
                    or "\\" in name or ":" in name or str(relative) != name or name in entries):
                raise ValueError(f"Invalid or duplicate native runtime path: {name!r}")
            is_native_notice = (
                len(relative.parts) > 2
                and relative.parts[0] == "THIRD_PARTY_LICENSES"
                and relative.parts[1] not in {"RUST", "NATIVE_RUST"}
            )
            path = binary / Path(*relative.parts)
            if path.is_symlink() or not path.is_file():
                raise ValueError(f"Missing native runtime file: {name}")
            if is_native_notice:
                continue
            if sha256(path) != expected:
                raise ValueError(f"Native runtime file changed before notice preparation: {name}")
            entries[name] = expected
    for existing in list(runtime.rglob("*")):
        parts = existing.relative_to(runtime).parts
        if existing.is_file() and len(parts) > 1 and parts[0] not in {"RUST", "NATIVE_RUST"}:
            existing.unlink()
    for relative, source_file in notices.items():
        destination = runtime / Path(*PurePosixPath(relative).parts)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(source_file.read_bytes())

    # Keep this manifest scoped to the native artifact. Windows-generated app
    # reports and other files added later have their own packaging lifecycle.
    for relative, source_file in sorted(notices.items()):
        name = f"THIRD_PARTY_LICENSES/{relative}"
        entries[name] = sha256(runtime / Path(*PurePosixPath(relative).parts))
    lines = ["path\tsha256", *(f"{name}\t{entries[name]}" for name in sorted(entries))]
    runtime_manifest.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    return len(notices)


def write_portable_zip(binary, output):
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for path in sorted(binary.rglob("*")):
            if path.is_symlink():
                raise ValueError(f"Portable package contains a symbolic link: {path}")
            if path.is_file():
                archive.write(path, path.relative_to(binary).as_posix())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--native-source", type=Path, required=True)
    parser.add_argument("--portable-zip", type=Path)
    args = parser.parse_args()
    count = prepare(args.binary, args.native_source)
    print(f"Prepared {count} native license notices from the captured source manifest")
    if args.portable_zip:
        write_portable_zip(args.binary, args.portable_zip)
        print(f"Wrote audited portable package: {args.portable_zip}")


if __name__ == "__main__":
    main()
