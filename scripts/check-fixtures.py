"""Verify the actual native-test inputs against their recorded provenance hashes."""
import hashlib
import json
from pathlib import Path

directory = Path(__file__).resolve().parents[1] / 'tests/fixtures'
manifest = json.loads((directory / 'manifest.json').read_text(encoding='utf-8'))
for name, metadata in manifest.items():
    path = directory / name
    assert path.is_file(), f'Missing fixture: {name}'
    assert hashlib.sha256(path.read_bytes()).hexdigest() == metadata['sha256'], f'Fixture changed: {name}'
    assert metadata['license'] == 'CC0-1.0', f'Unexpected fixture license: {name}'
print(f'{len(manifest)} original fixture hashes verified')
