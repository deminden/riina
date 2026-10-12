#!/usr/bin/env python3
"""Assemble the app bundle metadata and executable required by Launch Services."""

from __future__ import annotations

import argparse
import csv
import plistlib
import shutil
import tempfile
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "config" / "default-file-types.csv"
OPEN_EXTENSIONS = ROOT / "config" / "macos-open-extensions.csv"
CATEGORY_PARENT = {
    "video": "public.movie",
    "audio": "public.audio",
    "playlist": "public.text",
}


def file_types() -> list[tuple[str, str, list[str]]]:
    records = []
    with MANIFEST.open(encoding="utf-8", newline="") as source:
        for row in csv.reader(source, delimiter="|"):
            if not row or row[0].startswith("#"):
                continue
            if len(row) != 3 or row[0] not in CATEGORY_PARENT:
                raise ValueError(f"Invalid media type row: {row!r}")
            category, identifier, extensions = row
            records.append((category, identifier, extensions.split(",")))
    if not records:
        raise ValueError("The media type manifest is empty")
    return records


def document_extensions() -> dict[str, list[str]]:
    result = {}
    with OPEN_EXTENSIONS.open(encoding="utf-8", newline="") as source:
        for row in csv.reader(source, delimiter="|"):
            if not row or row[0].startswith("#"):
                continue
            if len(row) != 2 or row[0] not in CATEGORY_PARENT:
                raise ValueError(f"Invalid document extension row: {row!r}")
            if row[0] in result:
                raise ValueError(f"Duplicate document type category: {row[0]}")
            result[row[0]] = row[1].split(",")
    if set(result) != set(CATEGORY_PARENT):
        raise ValueError("The document extension manifest must define every media category")
    return result


def make_info_plist(
    version: str,
    records: list[tuple[str, str, list[str]]],
    extensions_by_category: dict[str, list[str]],
) -> dict:
    document_types = []
    for category, name in (("video", "Video"), ("audio", "Audio"), ("playlist", "Playlist")):
        selected = [row for row in records if row[0] == category]
        document_types.append(
            {
                "CFBundleTypeName": f"Riina {name}",
                "CFBundleTypeRole": "Viewer",
                "LSHandlerRank": "Alternate",
                "LSItemContentTypes": [row[1] for row in selected],
                "CFBundleTypeExtensions": extensions_by_category[category],
            }
        )
    imported_types = [
        {
            "UTTypeIdentifier": identifier,
            "UTTypeDescription": f"Riina supported {category} type",
            "UTTypeConformsTo": [CATEGORY_PARENT[category]],
            "UTTypeTagSpecification": {"public.filename-extension": extensions},
        }
        for category, identifier, extensions in records
    ]
    return {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleExecutable": "riina",
        "CFBundleIdentifier": "org.riina.player",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": "Riina",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        "LSApplicationCategoryType": "public.app-category.video",
        "NSHighResolutionCapable": True,
        "CFBundleDocumentTypes": document_types,
        "UTImportedTypeDeclarations": imported_types,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path, help="built macOS riina executable")
    parser.add_argument("app_bundle", type=Path, help="new .app output path")
    arguments = parser.parse_args()

    executable = arguments.executable.resolve(strict=True)
    destination = arguments.app_bundle.resolve()
    if destination.suffix != ".app":
        parser.error("app_bundle must end in .app")
    if not executable.is_file():
        parser.error("executable must be a file")
    if destination.exists():
        parser.error(f"refusing to replace existing app bundle: {destination}")

    with (ROOT / "Cargo.toml").open("rb") as source:
        version = tomllib.load(source)["workspace"]["package"]["version"]
    records = file_types()
    extensions_by_category = document_extensions()
    destination.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(
        tempfile.mkdtemp(prefix=f".{destination.stem}-", dir=destination.parent)
    )
    staged_bundle = staging / destination.name
    try:
        macos = staged_bundle / "Contents" / "MacOS"
        macos.mkdir(parents=True)
        shutil.copy2(executable, macos / "riina")
        with (staged_bundle / "Contents" / "Info.plist").open("wb") as output:
            plistlib.dump(
                make_info_plist(version, records, extensions_by_category),
                output,
                sort_keys=False,
            )
        staged_bundle.rename(destination)
    finally:
        shutil.rmtree(staging, ignore_errors=True)
    print(destination)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
