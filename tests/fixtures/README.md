# Synthetic media

`styled-captions.ass` contains an original ASS style and two original timed cues
for style eligibility tests. No font is bundled. Regenerate with
`python scripts/generate-ass-fixture.py`; its CC0-1.0 provenance and hash are in
manifest.json. The generator is MIT.

`moving-square.avi` is original generated test data: 5 s, 320×180 MJPEG, 24 fps,
moving amber square, frame labels and orientation markers, 48 kHz stereo PCM with
a low-volume 440 Hz tone. Created by scripts/generate-fixtures.py using Pillow.
No third-party media is included. Fixture data is CC0-1.0; generator is MIT.
`secondary-captions.srt` contains two distinct original cues for simultaneous
subtitle-lane tests. Regenerate with `python scripts/generate-secondary-fixture.py`.
It is CC0-1.0 and its SHA-256 is recorded in manifest.json.
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

`long-silence.wav` is 1300 seconds of original silent mono PCM. Its compact
1000 Hz unsigned 8-bit samples allow ten-minute seek tests without audible
output or a large video fixture. Regenerate with
`python scripts/generate-long-seek-fixture.py`; it is CC0-1.0 with its hash in
manifest.json. It proves timeline behavior, not video seek accuracy.

`track-languages.mkv` preserves catalog.mkv's original video/audio/chapter bytes
and adds seven original UTF-8 subtitle tracks: English, German, French,
Traditional Chinese, Brazilian Portuguese, unspecified and an unknown code.
Legacy ISO 639-2 and LanguageBCP47 metadata exercise both naming paths. Generate
with `python scripts/generate-language-fixture.py`; fixture bytes are CC0-1.0,
generator is MIT, and the hash is in manifest.json. Native decoding confirms
mpv omits `und` but preserves the other language codes and both subtitle lanes.


`interlaced-tff.mkv` and `interlaced-bff.mkv` are original CC0 woven v210 clips.
Regenerate with `python scripts/generate-interlaced-fixtures.py` (standard library
only). Each contains 75 frames at 25 fps, with two moving fields 20 ms apart.
For an independent development oracle, install `av==16.0.1` with pip's
`--only-binary=:all: --no-deps --target tools/fixture-python` options, then run
`python scripts/check-interlaced-fixtures.py`. This ignored development dependency
is not bundled. The check decodes the woven fields and verifies 150 ordered
progressive bwdif outputs; it does not verify riina's rendered pixels.

The ignored native cadence test now writes 36 PNGs and a `captures.json` under
`tools/interlaced-native-test/<unique-run>`. Run
`python scripts/check-native-interlaced-captures.py <captures.json> --report <report.json>`
to compare actual saved native video pixels with the original field pattern.
The checker intentionally fails for current bottom-first temporal reversal; do
not treat cadence or a successfully written PNG as proof of correct field order.
This requires Pillow and does not capture GUI or monitor output.
