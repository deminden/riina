"""Read the mapping, sequence and literal-block subset used by workflow tests."""

from pathlib import Path
import re

WORKFLOWS = Path(__file__).resolve().parents[2] / ".github/workflows"
KEY = re.compile(r"([\w-]+):(?: |$)")


def workflow(name):
    return parse((WORKFLOWS / name).read_text(encoding="utf-8"))


def parse(source):
    lines = source.splitlines()
    cursor = 0

    def next_entry():
        nonlocal cursor
        while cursor < len(lines):
            line = lines[cursor].lstrip(" ")
            if line and not line.startswith("#"):
                return len(lines[cursor]) - len(line), line
            cursor += 1
        return -1, ""

    def block(indent):
        nonlocal cursor
        sequence = next_entry()[1].startswith("- ")
        entries = [] if sequence else {}
        while True:
            width, text = next_entry()
            if width < indent:
                return entries
            if width != indent:
                raise ValueError(f"Unexpected workflow indentation at line {cursor + 1}")
            if sequence:
                if not text.startswith("- "):
                    raise ValueError(f"Expected a sequence item at line {cursor + 1}")
                lines[cursor] = " " * (indent + 2) + text[2:]
                entries.append(block(indent + 2))
                continue

            match = KEY.match(text)
            if not match:
                raise ValueError(f"Expected a workflow key at line {cursor + 1}")
            key, raw = match[1], text[match.end():].strip()
            if key in entries:
                raise ValueError(f"Duplicate workflow key {key!r} at line {cursor + 1}")
            cursor += 1
            if raw == "|":
                entries[key] = literal(indent)
            elif raw.startswith("[") and raw.endswith("]"):
                entries[key] = [item.strip() for item in raw[1:-1].split(",") if item.strip()]
            elif raw:
                entries[key] = raw
            else:
                width, _ = next_entry()
                entries[key] = block(width) if width > indent else None

    def literal(indent):
        nonlocal cursor
        first = cursor
        widths = []
        while cursor < len(lines):
            line = lines[cursor]
            if line.strip():
                width = len(line) - len(line.lstrip(" "))
                if width <= indent:
                    break
                widths.append(width)
            cursor += 1
        width = min(widths, default=indent + 2)
        return "\n".join(line[width:] for line in lines[first:cursor]).rstrip("\n") + "\n"

    return block(0)


def named_step(job, name):
    step, = (step for step in job["steps"] if step.get("name") == name)
    return step


def action_step(job, action, **inputs):
    step, = (
        step for step in job["steps"]
        if step.get("uses", "").startswith(action + "@")
        and all(step.get("with", {}).get(key) == value for key, value in inputs.items())
    )
    return step
