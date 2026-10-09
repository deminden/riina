"""Original CC0 neutral HDR patches; no encoder or third-party media required.

Transfer functions: ITU-R BT.2100-2, Table 4/5. Container fields follow
https://www.matroska.org/technical/elements.html and codec_specs.html.
v210 packing/48-pixel stride is checked against FFmpeg's v210 decoder.
These fixtures test decoding/metadata, not calibrated display accuracy.
"""
from pathlib import Path
import hashlib
import json
import math
import struct

directory = Path(__file__).resolve().parents[1] / 'tests/fixtures'
WIDTH, HEIGHT, FPS, FRAMES = 192, 108, 8, 24


def vint(value):
    for size in range(1, 9):
        if value < (1 << (7 * size)) - 1:
            return (value | (1 << (7 * size))).to_bytes(size, 'big')
    raise ValueError('EBML size exceeds eight bytes')


def element(identifier, payload):
    return identifier.to_bytes((identifier.bit_length() + 7) // 8, 'big') + vint(len(payload)) + payload


def uint(identifier, value):
    return element(identifier, value.to_bytes(max(1, (value.bit_length() + 7) // 8), 'big'))


def text(identifier, value):
    return element(identifier, value.encode('utf-8'))


def pq(nits):
    m1, m2 = 2610 / 16384, 2523 / 32
    c1, c2, c3 = 3424 / 4096, 2413 / 128, 2392 / 128
    normalized = (nits / 10000) ** m1
    return ((c1 + c2 * normalized) / (1 + c3 * normalized)) ** m2


def hlg(scene):
    a = 0.17883277
    b = 1 - 4 * a
    c = 0.5 - a * math.log(4 * a)
    return math.sqrt(3 * scene) if scene <= 1 / 12 else a * math.log(12 * scene - b) + c


def frame(codes, number):
    pixels = bytearray()
    for row in range(HEIGHT):
        values = [codes[min(11, x // 16)] for x in range(WIDTH)]
        if row >= HEIGHT - 12:
            # Moving marker distinguishes a presented replacement from a frozen frame.
            values = [940 if number * 8 <= x < number * 8 + 8 else 64 for x in range(WIDTH)]
        for x in range(0, WIDTH, 6):
            y0, y1, y2, y3, y4, y5 = values[x:x + 6]
            pixels.extend(struct.pack('<4I', 512 | y0 << 10 | 512 << 20,
                                      y1 | 512 << 10 | y2 << 20,
                                      512 | y3 << 10 | 512 << 20,
                                      y4 | 512 << 10 | y5 << 20))
    assert len(pixels) == WIDTH * HEIGHT * 8 // 3
    return pixels


manifest_path = directory / 'manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
patches = {}
for name, transfer, units, values, curve in [
    ('hdr-pq.mkv', 16, 'cd/m2', [0, .01, .1, 1, 10, 100, 203, 400, 1000, 2000, 4000, 10000], pq),
    ('hdr-hlg.mkv', 18, 'relative scene light', [0, .001, .005, .01, .02, .05, 1 / 12, .15, .25, .4, .7, 1], hlg),
]:
    codes = [round(64 + 876 * curve(value)) for value in values]
    assert codes == sorted(codes) and codes[0] == 64 and codes[-1] == 940
    header = element(0x1A45DFA3, uint(0x4286, 1) + uint(0x42F7, 1)
                     + uint(0x42F2, 4) + uint(0x42F3, 8) + text(0x4282, 'matroska')
                     + uint(0x4287, 4) + uint(0x4285, 2))
    info = element(0x1549A966, uint(0x2AD7B1, 1_000_000)
                   + element(0x4489, struct.pack('>d', 3000.0))
                   + text(0x4D80, 'wiina HDR fixtures') + text(0x5741, 'wiina HDR fixtures'))
    bitmap = struct.pack('<IiiHH4sIiiII', 40, WIDTH, HEIGHT, 1, 20, b'v210',
                         WIDTH * HEIGHT * 8 // 3, 0, 0, 0, 0)
    colour = element(0x55B0, uint(0x55B1, 9) + uint(0x55B2, 10)
                     + uint(0x55B9, 1) + uint(0x55BA, transfer) + uint(0x55BB, 9))
    track = element(0xAE, uint(0xD7, 1) + uint(0x73C5, 1) + uint(0x83, 1)
                    + text(0x86, 'V_MS/VFW/FOURCC') + element(0x63A2, bitmap)
                    + uint(0x23E383, 1_000_000_000 // FPS)
                    + element(0xE0, uint(0xB0, WIDTH) + uint(0xBA, HEIGHT) + colour))
    blocks = bytearray(uint(0xE7, 0))
    for number in range(FRAMES):
        blocks.extend(element(0xA3, vint(1) + struct.pack('>hB', number * 1000 // FPS, 0x80)
                              + frame(codes, number)))
    payload = header + element(0x18538067, info + element(0x1654AE6B, track) + element(0x1F43B675, blocks))
    (directory / name).write_bytes(payload)
    manifest[name] = dict(sha256=hashlib.sha256(payload).hexdigest(), license='CC0-1.0',
                         origin='original synthetic generation', command='python scripts/generate-hdr-fixtures.py',
                         description=f'192x108 v210 10-bit neutral patches, BT.2020, transfer {transfer}, limited range; 3 s at 8 fps')
    patches[name] = dict(units=units, patch_values=values, luma_codes=codes,
                        neutral_chroma_code=512, marker='Bottom 12 rows: white 8-pixel stripe advances 8 pixels/frame')
patch_data = (json.dumps(patches, indent=2) + '\n').encode('utf-8')
(directory / 'hdr-patches.json').write_bytes(patch_data)
manifest['hdr-patches.json'] = dict(sha256=hashlib.sha256(patch_data).hexdigest(),
                                  license='CC0-1.0', origin='original synthetic generation',
                                  command='python scripts/generate-hdr-fixtures.py',
                                  description='Original patch values, quantized luma codes and marker geometry')
manifest_path.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
print('Generated original PQ and HLG fixtures')
