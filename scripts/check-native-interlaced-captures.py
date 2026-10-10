"""Check saved native video pixels against the original moving-field pattern.

This checks engine screenshot output, not the displayed UI or monitor pipeline.
Requires Pillow, also used by the original fixture generators.
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from PIL import Image


def check(manifest):
    entries = json.loads(manifest.read_text(encoding="utf-8"))
    fixtures = {item["fixture"] for item in entries}
    original = {"interlaced-tff.mkv", "interlaced-bff.mkv"}
    encoded = {"interlaced-mpeg2-tff.mkv", "interlaced-mpeg2-bff.mkv"}
    assert fixtures in (original, original | encoded), "Require paired field orders"
    assert Counter((item["fixture"], item["deinterlace"]) for item in entries) == {
        (name, mode): 6 if mode else 12 for name in fixtures for mode in (False, True)
    }, "All three control modes must be covered"
    observations = []
    for entry in entries:
        path = Path(entry["path"])
        with Image.open(path) as image:
            assert image.size == (96, 64)
            pixels = image.convert("RGB")
            rows = [[x for x in range(96) if pixels.getpixel((x, row))[0] > 128]
                    for row in (24, 25)]
        field = round(entry["position"] * 50)
        # Off retains the two woven temporal samples. On must present the
        # sample at the frame timestamp, independent of which row came first.
        if entry["deinterlace"]:
            expected = [8 + field * 2 % 64] * 2
        else:
            pair = [8 + field * 2 % 64, 8 + (field + 1) * 2 % 64]
            expected = pair if entry["fixture"].endswith("-tff.mkv") else pair[::-1]
        observed = [row[0] if row else None for row in rows]
        observations.append({
            "fixture": entry["fixture"], "deinterlace": entry["deinterlace"],
            "position": entry["position"], "expected_edges": expected,
            "observed_edges": observed, "bar_widths": list(map(len, rows)),
            "passed": observed == expected and all(len(row) == 12 for row in rows),
            "png_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        })
    failures = [item for item in observations if not item["passed"]]
    return {"scope": "Native saved video pixels; not GUI/monitor output",
            "captures": len(entries), "failures": len(failures),
            "observations": observations}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    report = check(args.manifest)
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"{report['captures']} native captures; {report['failures']} field-pattern failures")
    raise SystemExit(1 if report["failures"] else 0)
