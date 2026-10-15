"""Check published asset provenance without granting third-party rights."""
import hashlib
import json
import argparse
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
directory = root / "assets"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--staged", action="store_true", help="Check the exact Git index bytes before committing")
staged = parser.parse_args().staged


def read_asset(name):
    if staged:
        return subprocess.check_output(["git", "show", f":{name}"], cwd=root)
    content = (root / name).read_bytes()
    # The repository enforces LF in Git while Windows may check out CRLF under
    # core.autocrlf=true. Provenance hashes describe the canonical asset bytes.
    if b"\0" not in content:
        content = content.replace(b"\r\n", b"\n")
    return content


manifest = json.loads(read_asset("assets/provenance.json"))
recorded = set()
for asset in manifest["assets"]:
    name = asset["path"]
    path = (root / name).resolve()
    assert path.is_relative_to(directory.resolve()), f"Asset escapes its directory: {name}"
    assert name not in recorded, f"Duplicate asset record: {name}"
    assert asset["author"] and asset["license"] and asset["origin"], name
    assert hashlib.sha256(read_asset(name)).hexdigest() == asset["sha256"], name
    recorded.add(name)

asset_extensions = {".svg", ".png", ".jpg", ".jpeg", ".webp", ".ico", ".ttf", ".otf", ".woff", ".woff2"}
if staged:
    # Windows working files can use CRLF while Git stores LF. The publication
    # gate must hash the indexed bytes, rather than trusting a local-only pass.
    paths = subprocess.check_output(["git", "ls-files", "-z", "--", "assets"], cwd=root).decode("utf-8")
    present = {name for name in paths.split("\0") if Path(name).suffix.lower() in asset_extensions}
else:
    present = {
        path.relative_to(root).as_posix()
        for path in directory.rglob("*")
        if path.is_file() and path.suffix.lower() in asset_extensions
    }
assert present == recorded, f"Unrecorded or missing assets: {present ^ recorded}"
print(f"{len(recorded)} asset provenance records and hashes verified ({'Git index' if staged else 'checkout'})")
