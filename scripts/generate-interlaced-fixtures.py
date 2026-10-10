"""Original CC0 field-motion clips; the independent generator is MIT.

Matroska interlacing/field order: https://www.matroska.org/technical/elements.html
v210 packing: https://www.ffmpeg.org/doxygen/8.0/libavcodec_2v210dec_8c_source.html
No downloaded footage or encoder is used. Adjacent lines belong to different
temporal samples, so these are genuine woven fields, not relabelled progressive
frames. A 25 Hz frame clock gives exact millisecond timestamps at 50 fields/s.
"""
import hashlib
import json
from pathlib import Path
import struct

WIDTH, HEIGHT, FPS, FRAMES = 96, 64, 25, 75


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


def frame(number, top_first):
    pixels = bytearray()
    for row in range(HEIGHT):
        field = number * 2 + ((row % 2) if top_first else (1 - row % 2))
        left = 8 + field * 2 % 64
        luma = [940 if 8 <= row < HEIGHT - 8 and left <= x < left + 12 else 64
                for x in range(WIDTH)]
        for x in range(0, WIDTH, 6):
            y0, y1, y2, y3, y4, y5 = luma[x:x + 6]
            pixels.extend(struct.pack('<4I', 512 | y0 << 10 | 512 << 20,
                                      y1 | 512 << 10 | y2 << 20,
                                      512 | y3 << 10 | 512 << 20,
                                      y4 | 512 << 10 | y5 << 20))
    assert len(pixels) == WIDTH * HEIGHT * 8 // 3
    # Width is a multiple of 48: v210 needs no implicit row padding.
    stride = WIDTH * 8 // 3
    assert pixels[8 * stride:9 * stride] != pixels[9 * stride:10 * stride]
    return pixels


def fixture(top_first):
    header = element(0x1A45DFA3, uint(0x4286, 1) + uint(0x42F7, 1)
                     + uint(0x42F2, 4) + uint(0x42F3, 8) + text(0x4282, 'matroska')
                     + uint(0x4287, 4) + uint(0x4285, 2))
    info = element(0x1549A966, uint(0x2AD7B1, 1_000_000)
                   + element(0x4489, struct.pack('>d', 3000.0))
                   + text(0x4D80, 'riina original field fixtures')
                   + text(0x5741, 'riina original field fixtures'))
    bitmap = struct.pack('<IiiHH4sIiiII', 40, WIDTH, HEIGHT, 1, 20, b'v210',
                         WIDTH * HEIGHT * 8 // 3, 0, 0, 0, 0)
    colour = element(0x55B0, uint(0x55B1, 1) + uint(0x55B2, 10)
                     + uint(0x55B9, 1) + uint(0x55BA, 1) + uint(0x55BB, 1))
    video = element(0xE0, uint(0xB0, WIDTH) + uint(0xBA, HEIGHT)
                    + uint(0x9A, 1) + uint(0x9D, 9 if top_first else 14) + colour)
    track = element(0xAE, uint(0xD7, 1) + uint(0x73C5, 1) + uint(0x83, 1)
                    + text(0x86, 'V_MS/VFW/FOURCC') + element(0x63A2, bitmap)
                    + uint(0x23E383, 1_000_000_000 // FPS) + video)
    blocks = bytearray(uint(0xE7, 0))
    for number in range(FRAMES):
        blocks.extend(element(0xA3, vint(1) + struct.pack('>hB', number * 1000 // FPS, 0x80)
                              + frame(number, top_first)))
    return header + element(0x18538067, info + element(0x1654AE6B, track)
                            + element(0x1F43B675, blocks))


def main():
    directory = Path(__file__).resolve().parents[1] / 'tests/fixtures'
    manifest_path = directory / 'manifest.json'
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    for top_first, order in [(True, 'tff'), (False, 'bff')]:
        name = f'interlaced-{order}.mkv'
        payload = fixture(top_first)
        (directory / name).write_bytes(payload)
        manifest[name] = dict(sha256=hashlib.sha256(payload).hexdigest(), license='CC0-1.0',
                             origin='original synthetic generation',
                             command='python scripts/generate-interlaced-fixtures.py',
                             description=f'96x64 v210 neutral moving bar; {order}; 3 seconds at 25 frames/50 fields per second; alternating rows sampled 20 ms apart')
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    print('Generated original top-first and bottom-first woven-field fixtures')


if __name__ == '__main__':
    main()
