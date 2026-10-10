"""Generate original CC0 subtitle data for independent subtitle-lane tests."""
import hashlib
import json
from pathlib import Path

directory = Path(__file__).resolve().parents[1] / "tests/fixtures"
name = "secondary-captions.srt"
content = (
    "1\n00:00:00,000 --> 00:00:01,800\nSecondary lane: bonjour\n\n"
    "2\n00:00:02,000 --> 00:00:04,800\nSecondary chapter two\n"
).encode("utf-8")
(directory / name).write_bytes(content)
manifest_path = directory / "manifest.json"
raw_manifest = manifest_path.read_bytes()
manifest = json.loads(raw_manifest)
manifest[name] = {
    "sha256": hashlib.sha256(content).hexdigest(),
    "license": "CC0-1.0",
    "origin": "original synthetic generation",
    "command": "python scripts/generate-secondary-fixture.py",
    "description": "Original UTF-8 secondary subtitle text, two timed cues",
}
newline = "\r\n" if b"\r\n" in raw_manifest else "\n"
manifest_path.write_bytes((json.dumps(manifest, indent=2) + "\n").replace("\n", newline).encode("utf-8"))
print("Generated secondary-captions.srt")
