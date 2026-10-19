"""Validate traceability and visual evidence links; neither establishes parity."""
import argparse
import csv
import hashlib
import json
import re
from pathlib import Path, PurePosixPath, PureWindowsPath


ROOT = Path(__file__).resolve().parents[1]
BASELINE = "fbcff4df7183b335880396af0655f6bb89d46442"
VISUAL_ARCHIVE = "docs/history/visual-reference-audit-through-994f14c.csv"
VISUAL_ARCHIVE_SHA256 = "ea1074c3bab469bc7496111d295e10da6db3199cb4e316bf1a69b4cdaefaf43b"
REFERENCE_IDENTITY = ("id", "reference", "sha256")
VISUAL_FIELDS = set(REFERENCE_IDENTITY) | {
    "surface", "upstream", "appearance_claim", "macos_version", "display_scale", "capture_device",
    "reference_quality", "current_implementation", "verification", "current_evidence",
    "remaining_gaps", "next_action", "historical_evidence",
}


def read_rows(path, required):
    with path.open(encoding="utf-8", newline="") as file:
        reader = csv.DictReader(file)
        fields = reader.fieldnames or []
        if not required <= set(fields) or len(set(fields)) != len(fields):
            raise ValueError(f"Missing fields in {path.name}")
        rows = list(reader)
    if not rows or any(None in row or None in row.values() for row in rows):
        raise ValueError(f"Empty or malformed inventory: {path.name}")
    return rows


def validate_behaviors(root):
    required = {
        "id", "subsystem", "feature", "upstream_commit", "upstream_file", "symbol_or_lines",
        "upstream_default", "observable_behavior", "applicability", "windows_approach",
        "implementation_location", "dependencies", "test_ids", "implementation_status",
        "verification_status", "evidence", "remaining_difference",
    }
    rows = read_rows(root / "docs/parity.csv", required)
    if len({row["id"] for row in rows}) != len(rows):
        raise ValueError("Duplicate requirement ID")
    for row in rows:
        if row["upstream_commit"] != BASELINE:
            raise ValueError(f"Baseline changed: {row['id']}")
        for source_name in row["upstream_file"].split(";"):
            source = root / "reference/iina-upstream" / source_name.strip()
            if not source.is_file():
                raise ValueError(f"Missing source: {row['id']} {source}")
        if row["implementation_status"] in ("implemented", "partial"):
            for location in row["implementation_location"].split(";"):
                if not (root / location.strip()).is_file():
                    raise ValueError(f"Missing implementation: {row['id']} {location}")
        if row["verification_status"] != "pending":
            if not row["evidence"]:
                raise ValueError(f"Missing evidence: {row['id']}")
            if row["implementation_status"] == "implemented" and not row["test_ids"]:
                raise ValueError(f"Implemented behavior lacks a linked test: {row['id']}")
        if row["implementation_status"] == "exact":
            raise ValueError("Use implemented plus independent verification; exact is a parity conclusion")
    return len(rows)


def local_path(root, relative):
    if (not relative or "\\" in relative or PureWindowsPath(relative).drive
            or PurePosixPath(relative).is_absolute() or ".." in PurePosixPath(relative).parts):
        raise ValueError(f"Inventory path escapes the repository: {relative}")
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"Inventory path escapes the repository: {relative}")
    return path


def validate_visual_rows(rows, original_rows, root, require_references=False):
    ids = [row["id"] for row in rows]
    originals = {row["id"]: row for row in original_rows}
    if any(not identity.strip() for identity in ids):
        raise ValueError("Empty visual reference ID")
    if len(set(ids)) != len(ids):
        raise ValueError("Duplicate visual reference ID")
    if not originals.keys() <= set(ids):
        raise ValueError("Original visual reference IDs were dropped")
    checked_references = 0
    checked_evidence = set()
    for row in rows:
        identity = row["id"]
        original = originals.get(identity)
        # Capture identity stays fixed; annotations may improve as evidence arrives.
        if original and any(row[field] != original[field] for field in REFERENCE_IDENTITY):
            raise ValueError(f"Original reference identity changed: {identity}; add a new capture record")
        if original and row["historical_evidence"] != f"history/{Path(VISUAL_ARCHIVE).name}#{identity}":
            raise ValueError(f"Historical evidence link changed: {identity}")
        quality = row["reference_quality"]
        if quality not in {"usable", "near-duplicate", "failed", "missing", "context-only"}:
            raise ValueError(f"Unknown reference quality: {identity}")
        if quality != "missing" and not re.fullmatch(r"[0-9a-fA-F]{64}", row["sha256"]):
            raise ValueError(f"Missing reference image hash: {identity}")
        verification = row["verification"]
        if verification not in {"source-only", "partial-gui", "reference-blocked"}:
            raise ValueError(f"Unsupported visual acceptance claim: {identity}")
        if quality in {"failed", "missing"} and verification != "reference-blocked":
            raise ValueError(f"Unusable reference must remain blocked: {identity}")
        for field in ("current_implementation", "remaining_gaps", "next_action"):
            if not row[field].strip():
                raise ValueError(f"Missing {field}: {identity}")
        reference = local_path(root, row["reference"])
        if reference.is_file():
            if hashlib.sha256(reference.read_bytes()).hexdigest().lower() != row["sha256"].lower():
                raise ValueError(f"Reference image hash changed: {identity}")
            checked_references += 1
        elif require_references and quality != "missing":
            raise ValueError(f"Reference image unavailable: {identity}")
        evidence = re.findall(r"tests/visual/[A-Za-z0-9_.-]+\.json", row["current_evidence"])
        if verification == "partial-gui" and not evidence:
            raise ValueError(f"GUI claim lacks a linked evidence record: {identity}")
        for relative in evidence:
            if relative in checked_evidence:
                continue
            path = local_path(root, relative)
            if not path.is_file():
                raise ValueError(f"Missing GUI evidence: {relative}")
            data = json.loads(path.read_text(encoding="utf-8"))
            if not isinstance(data, dict) or not data.get("date") or not any(
                re.fullmatch(r"[0-9a-fA-F]{7,40}", str(data.get(field, "")))
                for field in ("code_revision", "source_revision", "source_base_revision")
            ):
                raise ValueError(f"GUI evidence lacks date/source identity: {relative}")
            checked_evidence.add(relative)
    return len(rows), checked_references, len(checked_evidence)


def validate_visual_ledger(root, require_references=False):
    archive = root / VISUAL_ARCHIVE
    # CSV is text: compare Git's canonical LF bytes on Windows as well as Unix.
    archive_hash = hashlib.sha256(archive.read_bytes().replace(b"\r\n", b"\n")).hexdigest()
    if archive_hash != VISUAL_ARCHIVE_SHA256:
        raise ValueError("Historical visual archive changed")
    original_rows = read_rows(archive, set(REFERENCE_IDENTITY))
    rows = read_rows(root / "docs/visual-reference-audit.csv", VISUAL_FIELDS)
    return validate_visual_rows(rows, original_rows, root, require_references)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-visual-references", action="store_true",
                        help="Require the local reference files as well as their recorded identities")
    args = parser.parse_args()
    count = validate_behaviors(ROOT)
    visual, images, evidence = validate_visual_ledger(ROOT, args.require_visual_references)
    print(f"{count} atomic requirements have valid traceability. Inventory is incomplete; no universal parity percentage.")
    print(f"{visual} visual identities and {evidence} evidence links verified; {images} local image hashes checked. No visual acceptance inferred.")


if __name__ == "__main__":
    main()
