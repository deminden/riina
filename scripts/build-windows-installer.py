"""Wrap a verified portable package without rebuilding or changing its payload."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


ROOT = Path(__file__).resolve().parents[1]
INNO_VERSION = "6.7.3"
INNO_SHA256 = "9c73c3bae7ed48d44112a0f48e66742c00090bdb5bef71d9d3c056c66e97b732"
COMPILER_FILES = {
    "ISCC.exe": "0a8757031b33777e4c9cbffee40f11a5062b36d25cbe144c1db73b6102b80ad7",
    "ISCmplr.dll": "85a1e3090d3a5b85319f001b7c8f9ecfad45f37eff030a67bbe29ef58b7aa2c3",
}


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def payload_files(package):
    required = {"riina.exe", "libmpv-2.dll", "LICENSE", "LICENSE-GPL-3.0.txt",
                "SOURCE-COMMIT.txt", "SOURCE-ACCESS.md", "SOURCE-MANIFEST.tsv",
                "RUNTIME-MANIFEST.tsv", "THIRD_PARTY_NOTICES.md"}
    files = {}
    folded = set()
    for path in sorted(package.rglob("*")):
        if path.is_symlink() or path.is_junction():
            raise ValueError("Installer payload must not contain links")
        if path.is_dir():
            continue
        relative = path.relative_to(package).as_posix()
        # Inno expands brace constants, while quotes/newlines delimit file rows.
        if (any(character in relative for character in '{}";\r\n:')
                or relative.casefold() in folded
                or path.name.casefold().startswith("unins")
                or path.name.casefold() in {"opengl32.dll", "libgallium_wgl.dll"}):
            raise ValueError(f"Unsafe or test-only installer payload: {relative}")
        folded.add(relative.casefold())
        files[relative] = digest(path)
    if not required <= files.keys():
        raise ValueError("Installer payload is missing required release files")
    return files


def build(package, output, compiler, version):
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("Expected a semantic release version")
    package, output = package.resolve(), output.resolve()
    if package == output or package in output.parents:
        raise ValueError("Installer output must be outside the portable payload")
    files = payload_files(package)
    source = (package / "SOURCE-COMMIT.txt").read_text(encoding="utf-8-sig").strip()
    if not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ValueError("Installer payload has no exact source commit")
    for name, expected in COMPILER_FILES.items():
        if digest(compiler.parent / name) != expected:
            raise ValueError(f"Installer requires reviewed Inno Setup {INNO_VERSION}: {name}")
    output.mkdir(parents=True, exist_ok=True)
    listing = output / "installer-files.iss"
    rows = []
    for relative in files:
        path = Path(relative)
        destination = "{app}" + ("\\" + str(path.parent) if len(path.parts) > 1 else "")
        rows.append(f'Source: "{package / path}"; DestDir: "{destination}"; Flags: ignoreversion')
    listing.write_text("\n".join(rows) + "\n", encoding="utf-8-sig")
    subprocess.run([str(compiler), f"/DPackageDir={package}", f"/DOutputDir={output}",
                    f"/DAppVersion={version}", f"/DPayloadList={listing}",
                    str(ROOT / "packaging/windows/riina.iss")], check=True)
    installer = output / f"riina-{version}-windows-x86_64-setup.exe"
    report = {"version": version, "source_commit": source,
              "packaging_commit": subprocess.check_output(
                  ["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip(),
              "compiler": f"Inno Setup {INNO_VERSION}",
              "compiler_download_sha256": INNO_SHA256,
              "installer": installer.name, "installer_sha256": digest(installer),
              "payload_files": files,
              "scope": "Installer build inventory; installed playback, upgrade and uninstall require separate verification."}
    (output / "INSTALLER-VERIFICATION.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in report.items() if key != "payload_files"}, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    build(args.package, args.output, args.compiler.resolve(), args.version)


if __name__ == "__main__":
    main()
