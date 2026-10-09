"""Check published asset provenance without granting third-party rights."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
directory = root / "assets"
manifest = json.loads((directory / "provenance.json").read_text(encoding="utf-8"))
recorded = set()
for asset in manifest["assets"]:
    name = asset["path"]
    path = (root / name).resolve()
    assert path.is_relative_to(directory.resolve()), f"Asset escapes its directory: {name}"
    assert name not in recorded, f"Duplicate asset record: {name}"
    assert asset["author"] and asset["license"] and asset["origin"], name
    assert hashlib.sha256(path.read_bytes()).hexdigest() == asset["sha256"], name
    recorded.add(name)

asset_extensions = {".svg", ".png", ".jpg", ".jpeg", ".webp", ".ttf", ".otf", ".woff", ".woff2"}
present = {
    path.relative_to(root).as_posix()
    for path in directory.rglob("*")
    if path.is_file() and path.suffix.lower() in asset_extensions
}
assert present == recorded, f"Unrecorded or missing assets: {present ^ recorded}"
print(f"{len(recorded)} asset provenance records and hashes verified")
