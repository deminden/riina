"""Check actual saved pixels for active and hidden subtitle capture modes."""
import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageChops

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("directory", type=Path)
parser.add_argument("--report", type=Path, required=True)
args = parser.parse_args()
paths = {name: args.directory / f"{name}.png" for name in ("video", "subtitles", "hidden")}
images = {}
for name, path in paths.items():
    with Image.open(path) as image:
        assert image.size == (320, 180), (name, image.size)
        images[name] = image.convert("RGB")
visible = ImageChops.difference(images["video"], images["subtitles"])
hidden = ImageChops.difference(images["video"], images["hidden"])
bounds = visible.getbbox()
# Font rasterization varies by host. Require actual cue pixels in the lower
# frame, while enforcing exact equality when the same cue is hidden.
assert bounds is not None and bounds[1] >= 120, bounds
assert hidden.getbbox() is None, "Hidden subtitles changed video-only pixels"
pixels = visible.tobytes()
changed = sum(any(pixels[index:index + 3]) for index in range(0, len(pixels), 3))
assert changed >= 100, changed
report = {
    "scope": "Native saved SDR pixels; not GUI, reference font parity or HDR output",
    "directory": args.directory.as_posix(),
    "size": [320, 180],
    "active_cue_bounds": bounds,
    "active_cue_changed_pixels": changed,
    "hidden_matches_video": True,
    "sha256": {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in paths.items()},
    "passed": True,
    "published": False,
}
args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(f"Active cue changes {changed} pixels at {bounds}; hidden capture matches video exactly")
