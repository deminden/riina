"""Supply software OpenGL only to a disposable hosted GUI-test directory."""
import argparse
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile
import urllib.request


VERSION = "26.0.3"
ARCHIVE_SHA256 = "de1ec45c906164de2c046288a8969e83cf2d51d031dc8dfd1f9371bb31cd6755"
URL = (f"https://github.com/pal1000/mesa-dist-win/releases/download/{VERSION}/"
       f"mesa3d-{VERSION}-release-msvc.7z")
FILES = ("opengl32.dll", "libgallium_wgl.dll")


def prepare(archive, application):
    if hashlib.sha256(archive.read_bytes()).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("CI Mesa archive differs from the reviewed checksum")
    if not (application / "riina.exe").is_file():
        raise ValueError("Expected an extracted test application")
    if any((application / name).exists() for name in FILES):
        raise ValueError("Refusing to replace existing graphics libraries")
    # Extract only these two self-contained x64 DLLs. Do not install drivers,
    # execute deployment scripts, or add anything to the release ZIP.
    with tempfile.TemporaryDirectory(prefix="riina-ci-mesa-") as directory:
        subprocess.run(["tar", "-xf", str(archive.resolve()), "-C", directory,
                        *(f"x64/{name}" for name in FILES)], check=True)
        sources = [Path(directory) / "x64" / name for name in FILES]
        if not all(path.is_file() and not path.is_symlink() for path in sources):
            raise ValueError("CI Mesa archive lacks its expected x64 drivers")
        for path in sources:
            shutil.copyfile(path, application / path.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--application", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    args = parser.parse_args()
    if not args.archive.exists():
        args.archive.parent.mkdir(parents=True, exist_ok=True)
        # Partial downloads never become a usable cache entry.
        temporary = args.archive.with_suffix(".download")
        with urllib.request.urlopen(URL, timeout=90) as incoming, temporary.open("xb") as output:
            shutil.copyfileobj(incoming, output)
        if hashlib.sha256(temporary.read_bytes()).hexdigest() != ARCHIVE_SHA256:
            raise ValueError("Downloaded CI Mesa archive checksum differs")
        temporary.rename(args.archive)
    prepare(args.archive, args.application)
    print(f"CI-only Mesa {VERSION} staged; release archive and application binary unchanged")


if __name__ == "__main__":
    main()
