"""Add original multilingual subtitle tracks to the original catalog fixture.

Matroska TrackEntry Language/LanguageBCP47 and BlockGroup/BlockDuration elements
follow https://www.matroska.org/technical/elements.html. Existing MJPEG/PCM bytes
are preserved, so this requires neither a transcoder nor downloaded media.
"""
from pathlib import Path
import hashlib
import json
import struct

root = Path(__file__).resolve().parents[1]
directory = root / 'tests/fixtures'


def vint(value):
    for length in range(1, 9):
        if value < (1 << (7 * length)) - 1:
            return (value | (1 << (7 * length))).to_bytes(length, 'big')
    raise ValueError('EBML value too large')


def element(identifier, body):
    return identifier.to_bytes((identifier.bit_length() + 7) // 8, 'big') + vint(len(body)) + body


def uint(identifier, value):
    return element(identifier, value.to_bytes(max(1, (value.bit_length() + 7) // 8), 'big'))


def text(identifier, value):
    return element(identifier, value.encode('utf-8'))


def elements(data):
    offset = 0
    while offset < len(data):
        id_length = 9 - data[offset].bit_length()
        assert 1 <= id_length <= 4
        identifier = int.from_bytes(data[offset:offset + id_length], 'big')
        offset += id_length
        size_length = 9 - data[offset].bit_length()
        assert 1 <= size_length <= 8
        size = int.from_bytes(data[offset:offset + size_length], 'big') & ((1 << (7 * size_length)) - 1)
        offset += size_length
        assert offset + size <= len(data)
        yield identifier, data[offset:offset + size]
        offset += size


tracks = [
    ('English captions', 'eng', None),
    ('German captions', 'ger', None),
    ('French captions', 'fre', None),
    ('Traditional Chinese captions', 'zho', 'zh-Hant'),
    ('Brazilian Portuguese captions', 'por', 'pt-BR'),
    ('Unspecified captions', 'und', None),
    ('Unknown captions', 'qzz', None),
]
subtitle_entries = bytearray()
subtitle_blocks = bytearray()
for number, (title, language, bcp47) in enumerate(tracks, 3):
    entry = (uint(0xD7, number) + uint(0x73C5, number) + uint(0x83, 17)
             + uint(0x88, 0) + text(0x86, 'S_TEXT/UTF8')
             + text(0x536E, title) + text(0x22B59C, language))
    if bcp47:
        entry += text(0x22B59D, bcp47)
    subtitle_entries.extend(element(0xAE, entry))
    payload = vint(number) + struct.pack('>hB', 0, 0) + title.encode('utf-8')
    subtitle_blocks.extend(element(0xA0, element(0xA1, payload) + uint(0x9B, 4800)))

source = list(elements((directory / 'catalog.mkv').read_bytes()))
assert [identifier for identifier, _ in source] == [0x1A45DFA3, 0x18538067]
segment = bytearray()
for identifier, body in elements(source[1][1]):
    if identifier == 0x1654AE6B:
        body += subtitle_entries
    elif identifier == 0x1F43B675:
        body += subtitle_blocks
    segment.extend(element(identifier, body))
name = 'track-languages.mkv'
(directory / name).write_bytes(element(*source[0]) + element(0x18538067, segment))
manifest_path = directory / 'manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
manifest[name] = dict(
    sha256=hashlib.sha256((directory / name).read_bytes()).hexdigest(),
    license='CC0-1.0', origin='original synthetic generation',
    command='python scripts/generate-language-fixture.py',
    description='Original MJPEG/PCM and seven UTF-8 tracks with ISO and BCP 47 language metadata',
)
manifest_path.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
print(f'Generated {name}')
