"""Summarize LLVM's source coverage for this workspace."""

from collections import defaultdict
from functools import cache
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


@cache
def source_path(filename):
    path = Path(filename).resolve()
    if path.is_relative_to(ROOT / "crates"):
        relative = path.relative_to(ROOT)
        if len(relative.parts) > 3 and relative.parts[2] == "src":
            return relative
    return None


def summarize(report):
    crates = defaultdict(lambda: [0, 0])
    functions = {}
    for data in report["data"]:
        for file in data["files"]:
            path = source_path(file["filename"])
            if path is not None:
                lines = file["summary"]["lines"]
                summary = crates[path.parts[1]]
                summary[0] += lines["covered"]
                summary[1] += lines["count"]
        for function in data["functions"]:
            if not function["regions"]:
                continue
            # Rust can put the signature and body in separate code regions.
            regions = [region for region in function["regions"] if region[7] == 0]
            if not regions:
                continue
            start, column, _, _, _, file_id, _, _ = regions[0]
            path = source_path(function["filenames"][file_id])
            if path is None:
                continue
            span = max(region[2] for region in regions if region[5] == file_id) - start + 1
            location = (str(path), start, column)
            previous_span, covered = functions.get(location, (0, False))
            functions[location] = (max(span, previous_span), covered or function["count"] != 0)

    print("Line coverage by crate (source files)")
    crate_names = sorted(path.name for path in (ROOT / "crates").iterdir() if (path / "Cargo.toml").is_file())
    for crate in crate_names:
        covered, total = crates[crate]
        rate = f"{covered / total:7.2%}" if total else "    n/a"
        print(f"  {crate:14} {covered:7,} / {total:7,}  {rate}")
    print("Largest uncovered function bodies (source line span)")
    uncovered = [(span, *location) for location, (span, covered) in functions.items() if not covered]
    for length, filename, start, _ in sorted(uncovered, key=lambda item: (-item[0], *item[1:]))[:10]:
        code = (ROOT / filename).read_text(encoding="utf-8").splitlines()
        declaration = code[start - 1].strip() if 0 < start <= len(code) else ""
        print(f"  {length:4}  {filename}:{start}  {declaration}")


if __name__ == "__main__":
    with Path(sys.argv[1]).open(encoding="utf-8") as source:
        summarize(json.load(source))
