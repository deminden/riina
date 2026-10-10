"""Generate original CC0 silent PCM for seeks beyond ten minutes."""
import hashlib
import json
from pathlib import Path
import wave

directory = Path(__file__).resolve().parents[1] / "tests/fixtures"
name = "long-silence.wav"
# Unsigned 8-bit silence keeps long-timeline tests small and inaudible.
with wave.open(str(directory / name), "wb") as audio:
    audio.setnchannels(1)
    audio.setsampwidth(1)
    audio.setframerate(1000)
    audio.writeframes(bytes([128]) * 1_300_000)

manifest_path = directory / "manifest.json"
raw_manifest = manifest_path.read_bytes()
manifest = json.loads(raw_manifest)
manifest[name] = {
    "sha256": hashlib.sha256((directory / name).read_bytes()).hexdigest(),
    "license": "CC0-1.0",
    "origin": "original synthetic generation",
    "command": "python scripts/generate-long-seek-fixture.py",
    "description": "1300 s silent mono unsigned 8-bit PCM at 1000 Hz for long relative seeks",
}
newline = "\r\n" if b"\r\n" in raw_manifest else "\n"
manifest_path.write_bytes((json.dumps(manifest, indent=2) + "\n").replace("\n", newline).encode("utf-8"))
print("Generated long-silence.wav")
