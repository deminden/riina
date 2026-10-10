"""Validate original native screenshot files and their decoded moving-field PNGs.

The native test writes formats.json after decoding each saved image with libmpv.
This checks saved pixels, not GUI previews, physical colors or HDR output.
"""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image


def check(manifest):
    items = json.loads(manifest.read_text(encoding='utf-8'))
    assert sorted(item['format'] for item in items) == ['avif', 'jpeg', 'jpg', 'jxl', 'png', 'webp']
    observations = []
    for item in items:
        encoded = Path(item['path']).read_bytes()
        kind = item['format']
        if kind == 'png':
            assert encoded.startswith(b'\x89PNG\r\n\x1a\n')
        elif kind in ('jpg', 'jpeg'):
            assert encoded.startswith(b'\xff\xd8') and encoded.endswith(b'\xff\xd9')
        elif kind == 'webp':
            assert encoded[:4] == b'RIFF' and encoded[8:12] == b'WEBP'
        elif kind == 'jxl':
            assert encoded.startswith((b'\xff\x0a', b'\0\0\0\x0cJXL \r\n\x87\n'))
        elif kind == 'avif':
            assert encoded[4:8] == b'ftyp' and b'avif' in encoded[:64]
        png = Path(item['decoded_png'])
        with Image.open(png) as image:
            assert image.size == (96, 64)
            pixels = image.convert('RGB')
            rows = [[x for x in range(96) if pixels.getpixel((x, row))[0] > 128] for row in (24, 25)]
        field = round(item['position'] * 50)
        expected = [8 + field * 2 % 64, 8 + (field + 1) * 2 % 64]
        assert [row[0] for row in rows] == expected and all(len(row) == 12 for row in rows), kind
        observations.append({'format': kind, 'position': item['position'], 'edges': expected,
                             'bar_widths': [12, 12], 'encoded_sha256': hashlib.sha256(encoded).hexdigest(),
                             'decoded_png_sha256': hashlib.sha256(png.read_bytes()).hexdigest()})
    return {'scope': 'Saved image signatures and native decoded PNG pixels; not GUI/HDR',
            'observations': observations, 'passed': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    report = check(args.manifest)
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print('All six native image formats retain expected dimensions and moving-field edges')
