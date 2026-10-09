"""Wrap original AVI packets in Matroska with chapters; generate UTF-8 subtitles.

Element IDs and SimpleBlock layout follow matroska.org/technical/elements.html
and notes.html; codec mappings follow technical/codec_specs.html. No transcoder
or downloaded media is needed, so the catalog test has reproducible inputs.
"""
from pathlib import Path
import hashlib
import json
import struct
import wave

root = Path(__file__).resolve().parents[1]
directory = root / 'tests/fixtures'

def vint(value):
    for length in range(1, 9):
        if value < (1 << (7 * length)) - 1:
            return (value | (1 << (7 * length))).to_bytes(length, 'big')
    raise ValueError('EBML size too large')

def element(identifier, body):
    return identifier.to_bytes((identifier.bit_length() + 7) // 8, 'big') + vint(len(body)) + body

def uint(identifier, value):
    return element(identifier, value.to_bytes(max(1, (value.bit_length() + 7) // 8), 'big'))

def text(identifier, value):
    return element(identifier, value.encode('utf-8'))

def riff_chunks(data, offset, end):
    while offset < end:
        tag = data[offset:offset + 4]
        size = struct.unpack_from('<I', data, offset + 4)[0]
        body = data[offset + 8:offset + 8 + size]
        yield tag, body
        offset += 8 + size + size % 2

avi = (directory / 'moving-square.avi').read_bytes()
movi = next(body[4:] for tag, body in riff_chunks(avi, 12, len(avi))
            if tag == b'LIST' and body[:4] == b'movi')
packets = list(riff_chunks(movi, 0, len(movi)))
assert len(packets) == 240

ebml = element(0x1A45DFA3, uint(0x4286, 1) + uint(0x42F7, 1)
               + uint(0x42F2, 4) + uint(0x42F3, 8) + text(0x4282, 'matroska')
               + uint(0x4287, 4) + uint(0x4285, 2))
info = element(0x1549A966, uint(0x2AD7B1, 1_000_000)
               + element(0x4489, struct.pack('>d', 5000.0))
               + text(0x4D80, 'wiina fixture generator') + text(0x5741, 'wiina fixture generator'))
video = element(0xAE, uint(0xD7, 1) + uint(0x73C5, 1) + uint(0x83, 1)
                + text(0x86, 'V_MJPEG') + uint(0x23E383, 41_666_667)
                + element(0xE0, uint(0xB0, 320) + uint(0xBA, 180)))
audio = element(0xAE, uint(0xD7, 2) + uint(0x73C5, 2) + uint(0x83, 2)
                + text(0x86, 'A_PCM/INT/LIT') + text(0x536E, 'Original tone')
                + text(0x22B59C, 'eng')
                + element(0xE1, element(0xB5, struct.pack('>d', 48000.0))
                          + uint(0x9F, 2) + uint(0x6264, 16)))
chapters = b''
for index, (start, end, title) in enumerate([(0, 2_000_000_000, 'Opening'),
                                            (2_000_000_000, 5_000_000_000, 'Moving square')], 1):
    chapters += element(0xB6, uint(0x73C4, index) + uint(0x91, start) + uint(0x92, end)
                        + element(0x80, text(0x85, title) + text(0x437C, 'eng')))
chapters = element(0x1043A770, element(0x45B9, chapters))
blocks = bytearray(uint(0xE7, 0))
pcm = bytearray()
for index, (tag, payload) in enumerate(packets):
    track = 1 if tag == b'00dc' else 2
    timestamp = round((index // 2) * 1000 / 24)
    blocks.extend(element(0xA3, vint(track) + struct.pack('>hB', timestamp, 0x80) + payload))
    if track == 2:
        pcm.extend(payload)
(directory / 'catalog.mkv').write_bytes(ebml + element(0x18538067, info
    + element(0x1654AE6B, video + audio) + chapters + element(0x1F43B675, bytes(blocks))))
with wave.open(str(directory / 'external-tone.wav'), 'wb') as output:
    output.setnchannels(2)
    output.setsampwidth(2)
    output.setframerate(48000)
    output.writeframes(pcm)
(directory / 'manual-captions.srt').write_text(
    '1\n00:00:00,000 --> 00:00:01,800\nwiina subtitle: café — Привет\n\n'
    '2\n00:00:02,000 --> 00:00:04,800\nChapter two — Moving square\n', encoding='utf-8')
manifest_path = directory / 'manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
for name, description in [('catalog.mkv', 'Original MJPEG/PCM packets with two named chapters'),
                          ('external-tone.wav', 'Original 5 s stereo PCM 440 Hz tone'),
                          ('manual-captions.srt', 'Original UTF-8 text, two timed cues')]:
    manifest[name] = dict(sha256=hashlib.sha256((directory / name).read_bytes()).hexdigest(),
                         license='CC0-1.0', origin='original synthetic generation',
                         command='python scripts/generate-catalog-fixtures.py', description=description)
manifest_path.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
print('Generated catalog.mkv, external-tone.wav and manual-captions.srt')
