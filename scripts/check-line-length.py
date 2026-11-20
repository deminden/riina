"""Check Rust source lines while ignoring the contents of string literals."""

import argparse
from collections import Counter
from collections.abc import Iterator
from hashlib import sha256
from pathlib import Path
import re
import subprocess
import sys


LIMIT = 120
ROOT = Path(__file__).resolve().parent.parent
ALLOW_LIST = "scripts/rust-line-length-allowlist.txt"
CODE_TOKEN = re.compile(
    r"""(?<!\w)(?:[bc]?r)(?P<hashes>\#{0,255})"|//|/\*|"|'(?:\\(?:u\{[\da-fA-F_]+\}|x[\da-fA-F]{2}|.)|[^'\\])'"""
)
BLOCK_TOKEN = re.compile(r"/\*|\*/")
STRING_TOKEN = re.compile(r'\\.|"')


def rust_line_widths(source: str) -> Iterator[tuple[str, int]]:
    """Count code and comments, preserving string delimiters and character literals."""
    block_depth = 0
    string_end = None
    for line in source.split('\n'):
        visible = []
        cursor = 0
        while cursor < len(line):
            if block_depth:
                marker = BLOCK_TOKEN.search(line, cursor)
                if not marker:
                    visible.append(line[cursor:])
                    break
                visible.append(line[cursor:marker.end()])
                block_depth += 1 if marker.group() == "/*" else -1
                cursor = marker.end()
            elif string_end is not None:
                delimiter, raw = string_end
                if not raw:
                    tokens = STRING_TOKEN.finditer(line, cursor)
                    close = next((token.start() for token in tokens if token.group() == '"'), -1)
                else:
                    close = line.find(delimiter, cursor)
                if close == -1:
                    break
                cursor = close + len(delimiter)
                visible.append(line[close:cursor])
                string_end = None
            else:
                marker = CODE_TOKEN.search(line, cursor)
                if not marker or marker.group() == "//":
                    visible.append(line[cursor:])
                    break
                visible.append(line[cursor:marker.end()])
                cursor = marker.end()
                if marker.group() == "/*":
                    block_depth = 1
                elif marker.group("hashes") is not None:
                    string_end = ('"' + marker.group("hashes"), True)
                elif marker.group() == '"':
                    string_end = ('"', False)
        yield line, len("".join(visible).expandtabs(4))


def baseline_entries(contents: str) -> Counter[str]:
    return Counter(line for line in contents.splitlines() if line and not line.startswith("#"))


def collect_violations(root: Path) -> list[tuple[str, int, int, str]]:
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "*.rs"],
        cwd=root,
    ).decode("utf-8")
    violations = []
    for name in sorted(set(filter(None, paths.split("\0")))):
        path = root / name
        if not path.is_file():
            continue
        for number, (line, width) in enumerate(rust_line_widths(path.read_text(encoding="utf-8")), 1):
            if width > LIMIT:
                key = f"{name}\t{sha256(line.strip().encode('utf-8')).hexdigest()}"
                violations.append((name, number, width, key))
    return violations


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", action="store_true", help="print current exception keys")
    parser.add_argument("--baseline", help="Git revision against which the allow-list may only shrink")
    args = parser.parse_args()
    violations = collect_violations(ROOT)
    if args.report:
        print("\n".join(sorted(key for _, _, _, key in violations)))
        return 0

    entries = baseline_entries((ROOT / ALLOW_LIST).read_text(encoding="utf-8"))
    previous = entries
    if args.baseline:
        exists = subprocess.check_output(
            ["git", "ls-tree", args.baseline, "--", ALLOW_LIST], cwd=ROOT, text=True,
        )
        if exists:
            contents = subprocess.check_output(
                ["git", "show", f"{args.baseline}:{ALLOW_LIST}"], cwd=ROOT, text=True,
            )
            previous = baseline_entries(contents)
    return compare_and_check(entries, previous, violations)


def compare_and_check(
    entries: Counter[str],
    previous: Counter[str],
    violations: list[tuple[str, int, int, str]],
) -> int:
    if added := entries - previous:
        for key in sorted(added):
            print(f"New exception in {ALLOW_LIST}: {key}", file=sys.stderr)
        return 1

    remaining = entries.copy()
    failed = False
    for name, number, width, key in violations:
        if remaining[key]:
            remaining[key] -= 1
        else:
            print(f"{name}:{number} has {width} columns outside strings (limit {LIMIT})", file=sys.stderr)
            failed = True
    for key in sorted(+remaining):
        print(f"Remove the unused exception from {ALLOW_LIST}: {key}", file=sys.stderr)
        failed = True
    return int(failed)


if __name__ == "__main__":
    sys.exit(main())
