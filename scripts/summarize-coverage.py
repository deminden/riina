"""Summarize LLVM's source coverage for this workspace."""

from collections import defaultdict
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


def source_path(filename):
    path = Path(filename).resolve()
    if path.is_relative_to(ROOT / "crates"):
        relative = path.relative_to(ROOT)
        if len(relative.parts) > 3 and relative.parts[2] == "src":
            return relative
    return None


def summarize(report):
    crates = defaultdict(lambda: [0, 0])
    uncovered = set()
    for data in report["data"]:
        for file in data["files"]:
            path = source_path(file["filename"])
            if path is not None:
                lines = file["summary"]["lines"]
                summary = crates[path.parts[1]]
                summary[0] += lines["covered"]
                summary[1] += lines["count"]
        for function in data["functions"]:
            if function["count"] != 0 or not function["regions"]:
                continue
            # LLVM provides the function body as its first code region.
            start, _, end, _, _, file_id, _, kind = function["regions"][0]
            path = source_path(function["filenames"][file_id])
            if path is not None and kind == 0:
                uncovered.add((end - start + 1, str(path), start))

    print("Line coverage by crate (source files)")
    for crate, (covered, total) in sorted(crates.items()):
        print(f"  {crate:14} {covered:7,} / {total:7,}  {covered / max(total, 1):7.2%}")
    print("Largest uncovered function bodies (source line span)")
    for length, filename, start in sorted(uncovered, key=lambda item: (-item[0], item[1], item[2]))[:10]:
        code = (ROOT / filename).read_text(encoding="utf-8").splitlines()
        declaration = code[start - 1].strip() if 0 < start <= len(code) else ""
        print(f"  {length:4}  {filename}:{start}  {declaration}")


if __name__ == "__main__":
    with Path(sys.argv[1]).open(encoding="utf-8") as source:
        summarize(json.load(source))
