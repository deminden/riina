"""Generate original CC0 ASS data for subtitle-style eligibility tests."""
import hashlib
import json
from pathlib import Path

directory = Path(__file__).resolve().parents[1] / "tests/fixtures"
name = "styled-captions.ass"
content = """[Script Info]
ScriptType: v4.00+
PlayResX: 320
PlayResY: 180

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,18,&H00FFFFFF,&H00FFFFFF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,1,0,2,8,8,8,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:01.80,Default,,0,0,0,,Styled first cue
Dialogue: 0,0:00:02.00,0:00:04.80,Default,,0,0,0,,Styled second cue
""".encode("utf-8")
(directory / name).write_bytes(content)
manifest_path = directory / "manifest.json"
raw = manifest_path.read_bytes()
manifest = json.loads(raw)
manifest[name] = {
    "sha256": hashlib.sha256(content).hexdigest(),
    "license": "CC0-1.0",
    "origin": "original synthetic generation",
    "command": "python scripts/generate-ass-fixture.py",
    "description": "Original ASS subtitle style and two timed cues; no bundled font",
}
newline = "\r\n" if b"\r\n" in raw else "\n"
manifest_path.write_bytes((json.dumps(manifest, indent=2) + "\n").replace("\n", newline).encode("utf-8"))
print(f"Generated {name}")
