# Synthetic media

`moving-square.avi` is original generated test data: 5 s, 320×180 MJPEG, 24 fps,
moving amber square, frame labels and orientation markers, 48 kHz stereo PCM with
a low-volume 440 Hz tone. Created by scripts/generate-fixtures.py using Pillow.
No third-party media is included. Fixture data is CC0-1.0; generator is MIT.
Regenerate with `python scripts/generate-fixtures.py`; record SHA-256 in manifest.json.

`catalog.mkv` wraps those same original JPEG/PCM packets in Matroska with two named
chapters at 0 and 2 s. `external-tone.wav` contains the original stereo tone;
`manual-captions.srt` provides two original UTF-8 cues including café and Привет.
Regenerate these with `python scripts/generate-catalog-fixtures.py` after generating
the AVI. The generator follows the official Matroska element/codec specifications.
These fixtures are CC0-1.0; all hashes are in manifest.json. No downloaded footage,
transcoder, native executable, or third-party subtitle text is used.
`hdr-pq.mkv` and `hdr-hlg.mkv` contain original 192×108 neutral patches and a moving
marker, 3 s at 8 fps, uncompressed v210 10-bit 4:2:2. Matroska declares BT.2020,
nonconstant-luminance matrix, limited range and PQ/HLG transfer. Regenerate with
`python scripts/generate-hdr-fixtures.py`. `hdr-patches.json` records patch inputs
and 10-bit luma codes. No audio or downloaded encoder is used. The tests verify
native decoding and effective color parameters, not measured display luminance.
mpv may infer peak luminance; these files do not carry mastering-display metadata.

Encoding equations: [ITU-R BT.2100-2](https://www.itu.int/rec/R-REC-BT.2100-2-201807-S/en).
Container mappings: [Matroska](https://www.matroska.org/technical/codec_specs.html).
Packing: [FFmpeg v210 decoder](https://www.ffmpeg.org/doxygen/8.0/libavcodec_2v210dec_8c_source.html).
All original fixture bytes and patch metadata are hashed in manifest.json.
Compressed HDR, malformed containers and network fixtures remain pending.
