"""Original CC0 MPEG-2 field-motion fixtures; generator is independently MIT.

Requires the ignored development PyAV 16.0.1 wheel used by the fixture oracle.
The bitstream carries field order, allowing comparison with container-only v210.
"""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/fixture-python"))
import av


def main():
    assert av.__version__ == "16.0.1", "Use the pinned fixture encoder"
    directory = ROOT / "tests/fixtures"
    manifest_path = directory / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    for order in ("tff", "bff"):
        name = f"interlaced-mpeg2-{order}.mkv"
        with av.open(str(directory / f"interlaced-{order}.mkv")) as source:
            with av.open(str(directory / name), "w", format="matroska",
                         options={"fflags": "+bitexact"}) as output:
                stream = output.add_stream("mpeg2video", rate=25)
                stream.width, stream.height, stream.pix_fmt = 96, 64, "yuv420p"
                stream.bit_rate, stream.gop_size, stream.max_b_frames = 1_000_000, 1, 0
                stream.options = {"flags": "+ildct+ilme+bitexact", "threads": "1",
                                  "field_order": "tt" if order == "tff" else "bb",
                                  "qmin": "2", "qmax": "2"}
                for frame in source.decode(video=0):
                    for packet in stream.encode(frame.reformat(format="yuv420p")):
                        output.mux(packet)
                for packet in stream.encode():
                    output.mux(packet)
        manifest[name] = {"sha256": hashlib.sha256((directory / name).read_bytes()).hexdigest(),
                          "license": "CC0-1.0", "origin": "original synthetic generation",
                          "command": "python scripts/generate-encoded-interlaced-fixtures.py",
                          "description": f"Original woven {order} fields encoded as MPEG-2; 96x64; 75 frames; 25 fps; bitstream field order"}
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print("Generated both original MPEG-2 field-order fixtures")


if __name__ == "__main__":
    main()
