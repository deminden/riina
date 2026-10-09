"""Validate traceability, without treating missing inventory as completed parity."""
import csv
from pathlib import Path

root = Path(__file__).resolve().parents[1]
required = {
    'id', 'subsystem', 'feature', 'upstream_commit', 'upstream_file', 'symbol_or_lines',
    'upstream_default', 'observable_behavior', 'applicability', 'windows_approach',
    'implementation_location', 'dependencies', 'test_ids', 'implementation_status',
    'verification_status', 'evidence', 'remaining_difference',
}
with (root / 'docs/parity.csv').open(encoding='utf-8', newline='') as file:
    reader = csv.DictReader(file)
    assert required <= set(reader.fieldnames), 'Missing parity fields'
    rows = list(reader)
assert len({row['id'] for row in rows}) == len(rows), 'Duplicate requirement ID'
baseline = 'fbcff4df7183b335880396af0655f6bb89d46442'
for row in rows:
    assert row['upstream_commit'] == baseline, f"Baseline changed: {row['id']}"
    source = root / 'reference/iina-upstream' / row['upstream_file']
    assert source.is_file(), f"Missing source: {row['id']} {source}"
    if row['implementation_status'] in ('implemented', 'partial'):
        assert (root / row['implementation_location']).is_file(), f"Missing implementation: {row['id']}"
    if row['verification_status'] != 'pending':
        assert row['test_ids'] and row['evidence'], f"Missing evidence: {row['id']}"
    if row['implementation_status'] == 'exact':
        raise AssertionError('Use implemented plus independent verification; exact is a parity conclusion')
print(f'{len(rows)} initial atomic requirements have valid traceability. Inventory is incomplete; no universal parity percentage.')
