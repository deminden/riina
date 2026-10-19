"""Keep visual reference identity, evidence links and acceptance limits intact."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


spec = importlib.util.spec_from_file_location("parity", Path(__file__).parents[1] / "check-parity.py")
parity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(parity)


class VisualLedgerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.image = self.root / "reference/capture.png"
        self.image.parent.mkdir()
        self.image.write_bytes(b"original reference bytes")
        self.evidence = self.root / "tests/visual/check.json"
        self.evidence.parent.mkdir(parents=True)
        self.evidence.write_text(json.dumps({"date": "2026-10-19", "source_revision": "a" * 40}))
        self.row = {field: "recorded" for field in parity.VISUAL_FIELDS}
        self.row.update({
            "id": "REF-test", "reference": "reference/capture.png",
            "sha256": hashlib.sha256(self.image.read_bytes()).hexdigest(),
            "reference_quality": "usable", "verification": "partial-gui",
            "current_evidence": "tests/visual/check.json (scope only)",
            "historical_evidence": f"history/{Path(parity.VISUAL_ARCHIVE).name}#REF-test",
        })
        self.original = copy.deepcopy(self.row)

    def check(self, rows=None, require=False):
        return parity.validate_visual_rows(
            [self.row] if rows is None else rows, [self.original], self.root, require,
        )

    def test_valid_scoped_evidence_and_case_insensitive_image_digest(self):
        self.assertEqual(self.check(require=True), (1, 1, 1))
        self.row["sha256"] = self.row["sha256"].upper()
        self.original["sha256"] = self.row["sha256"]
        self.assertEqual(self.check(), (1, 1, 1))

    def test_real_repository_ledger_and_archived_identity(self):
        records, _, evidence = parity.validate_visual_ledger(parity.ROOT)
        self.assertGreaterEqual(records, 69)
        self.assertGreater(evidence, 0)

    def test_local_references_are_optional_in_ci_but_required_for_comparison(self):
        self.image.unlink()
        self.assertEqual(self.check(), (1, 0, 1))
        with self.assertRaisesRegex(ValueError, "Reference image unavailable"):
            self.check(require=True)

    def test_image_byte_changes_are_detected(self):
        self.image.write_bytes(b"replacement pixels")
        with self.assertRaisesRegex(ValueError, "image hash changed"):
            self.check()

    def test_original_rows_cannot_be_dropped_duplicated_or_reidentified(self):
        for rows, message in (([], "IDs were dropped"), ([self.row, self.row], "Duplicate")):
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                self.check(rows)
        for field in parity.REFERENCE_IDENTITY:
            with self.subTest(field=field), self.assertRaises(ValueError):
                row = copy.deepcopy(self.row)
                row[field] = "replacement"
                self.check([row])

    def test_new_reference_records_can_be_added_without_rewriting_old_ones(self):
        extra = copy.deepcopy(self.row)
        extra.update(id="REF-new", historical_evidence="")
        self.assertEqual(self.check([self.row, extra]), (2, 2, 1))

    def test_metadata_can_be_clarified_without_replacing_capture_identity(self):
        self.row["display_scale"] = "2 (user-confirmed)"
        self.row["appearance_claim"] = "Dark playback; light Preferences owner"
        self.assertEqual(self.check(), (1, 1, 1))

    def test_historical_archive_rewrites_are_rejected(self):
        archive = self.root / parity.VISUAL_ARCHIVE
        archive.parent.mkdir(parents=True)
        archive.write_bytes(b"rewritten historical observations")
        with self.assertRaisesRegex(ValueError, "Historical visual archive changed"):
            parity.validate_visual_ledger(self.root)

    def test_unknown_quality_and_acceptance_claims_are_rejected(self):
        for field, value in (("reference_quality", "perfect"), ("verification", "complete")):
            with self.subTest(field=field), self.assertRaises(ValueError):
                row = copy.deepcopy(self.row)
                row[field] = value
                self.check([row])

    def test_failed_or_missing_references_remain_blocked_despite_gui_evidence(self):
        for quality in ("failed", "missing"):
            with self.subTest(quality=quality), self.assertRaisesRegex(ValueError, "must remain blocked"):
                row = copy.deepcopy(self.row)
                row["reference_quality"] = quality
                self.check([row])
        self.row.update(reference_quality="missing", verification="reference-blocked")
        self.image.unlink()
        self.assertEqual(self.check(require=True), (1, 0, 1))

    def test_current_findings_and_history_link_must_remain_explicit(self):
        for field in ("current_implementation", "remaining_gaps", "next_action", "historical_evidence"):
            with self.subTest(field=field), self.assertRaises(ValueError):
                row = copy.deepcopy(self.row)
                row[field] = ""
                self.check([row])

    def test_gui_claim_requires_an_existing_report_with_date_and_source(self):
        self.row["current_evidence"] = "GUI tested"
        with self.assertRaisesRegex(ValueError, "lacks a linked"):
            self.check()
        self.row["current_evidence"] = "tests/visual/missing.json"
        with self.assertRaisesRegex(ValueError, "Missing GUI evidence"):
            self.check()
        self.row["current_evidence"] = "tests/visual/check.json"
        for data in ([], {}, {"date": "2026-10-19"}, {"date": "2026-10-19", "source_revision": "latest"}):
            with self.subTest(data=data), self.assertRaisesRegex(ValueError, "date/source identity"):
                self.evidence.write_text(json.dumps(data))
                self.check()

    def test_corrupt_reports_fail_instead_of_becoming_source_only(self):
        self.evidence.write_text("{broken")
        with self.assertRaises(json.JSONDecodeError):
            self.check()

    def test_paths_cannot_read_outside_the_repository_on_either_platform(self):
        for relative in ("../outside.png", "/outside.png", "C:/outside.png", "C:outside.png", "..\\outside.png", ""):
            with self.subTest(relative=relative), self.assertRaisesRegex(ValueError, "escapes"):
                parity.local_path(self.root, relative)

    def test_csv_missing_fields_duplicate_columns_and_malformed_rows_fail(self):
        path = self.root / "inventory.csv"
        for text in ("name\nvalue\n", "id,id\na,b\n", "id,name\na\n", "id\na,b\n", "id\n"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                path.write_text(text)
                parity.read_rows(path, {"id"})


if __name__ == "__main__":
    unittest.main()
