"""Original CC0 16-bit RGB PNG patches; independent MIT standard-library generator."""
import hashlib
import json
from pathlib import Path
import struct
import zlib


def chunk(kind, payload):
    return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))


def main():
    directory = Path(__file__).resolve().parents[1] / 'tests/fixtures'
    colors = [(0, 0, 0), (65535, 65535, 65535), (65535, 0, 0),
              (0, 65535, 0), (0, 0, 65535), (32768, 32768, 32768),
              (0, 65535, 65535), (65535, 0, 65535)]
    row = b'\0' + b''.join(struct.pack('>3H', *color) * 12 for color in colors)
    payload = (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 96, 64, 16, 2, 0, 0, 0))
               + chunk(b'IDAT', zlib.compress(row * 64, 9)) + chunk(b'IEND', b''))
    name = 'preview-rgb16.png'
    (directory / name).write_bytes(payload)
    manifest_path = directory / 'manifest.json'
    manifest = json.loads(manifest_path.read_text())
    manifest[name] = {'sha256': hashlib.sha256(payload).hexdigest(), 'license': 'CC0-1.0',
                      'origin': 'original synthetic generation',
                      'command': 'python scripts/generate-preview-fixture.py',
                      'description': '96x64 original eight-patch RGB PNG; 16 bits/channel; no HDR/colorimetry claim'}
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
    print('Generated original deep-RGB preview decoder fixture')


if __name__ == '__main__':
    main()
