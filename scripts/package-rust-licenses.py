"""Collect declared licenses and notice files for the Windows Rust package."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import urlparse
from urllib.request import Request, urlopen


LICENSE_NAMES = ("license", "copying", "notice", "copyright", "authors")
SPDX_FILENAMES = {
    "MIT": ("LICENSE-MIT", "LICENSE-MIT.txt", "MIT-LICENSE", "LICENSE", "LICENSE.md"),
    "Apache-2.0": ("LICENSE-APACHE", "LICENSE-APACHE.txt", "APACHE-LICENSE", "LICENSE", "LICENSE.md"),
    "BSL-1.0": ("LICENSE_1_0.txt", "LICENSE-BOOST", "LICENSE", "LICENSE.md"),
}
GPLV3_COMPATIBLE_GRANTS = {
    "0BSD", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "BSL-1.0", "CC0-1.0",
    "ISC", "LicenseRef-Slint-Royalty-free-2.0", "MIT", "MPL-2.0", "Unicode-3.0",
    "Unicode-DFS-2016", "Unlicense", "Zlib",
}
REVIEWED_DATA_GRANTS = {
    ("webpki-roots", "1.0.9", "CDLA-Permissive-2.0"): {
        "classification": "third-party data license",
        "rationale": (
            "This crate distributes derived Mozilla CA root certificate data from CCADB. "
            "The selected CDLA grant applies to that data, not as a software-license option "
            "for the combined application. Keep the crate's CDLA text and CCADB attribution."
        ),
        "source": "https://github.com/rustls/webpki-roots",
        "license_url": "https://cdla.dev/permissive-2-0/",
        "data_source": "https://www.ccadb.org/rootstores/usage",
        "attribution": (
            "The certificate data in webpki-roots is derived from the Common CA Database "
            "(CCADB). It is distributed under the Community Data License Agreement - "
            "Permissive, Version 2.0. See the accompanying LICENSE file and "
            "https://www.ccadb.org/rootstores/usage."
        ),
    },
}


def dependency_packages(metadata, root_name="riina"):
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    roots = [
        package["id"]
        for package in metadata["packages"]
        if package["name"] == root_name and package["id"] in metadata["workspace_members"]
    ]
    if len(roots) != 1:
        raise ValueError(f"Expected one workspace root named {root_name}; found {len(roots)}")

    visited = set()
    pending = roots[:]
    while pending:
        package_id = pending.pop()
        if package_id in visited:
            continue
        visited.add(package_id)
        node = nodes.get(package_id)
        if node is not None:
            pending.extend(dependency["pkg"] for dependency in node["deps"])
    return [packages[package_id] for package_id in sorted(visited)]


def choose_license(expression):
    """Record one explicit grant from Cargo's declared SPDX-style expression."""
    normalized = expression.replace("(", " ").replace(")", " ")
    if "LicenseRef-Slint-Royalty-free-2.0" in normalized:
        return "LicenseRef-Slint-Royalty-free-2.0"
    if " AND " in normalized:
        return " AND ".join(choose_license(part) for part in normalized.split(" AND "))
    options = re.split(r"\s+OR\s+|\s*/\s*", normalized)
    options = [option.strip() for option in options if option.strip()]
    preferred = ("MIT", "Apache-2.0", "ISC", "BSD-3-Clause", "BSD-2-Clause", "0BSD",
                 "CC0-1.0", "BSL-1.0", "Zlib", "Unicode-3.0", "MPL-2.0", "Unlicense")
    for license_id in preferred:
        if license_id in options:
            return license_id
    if len(options) == 1:
        return options[0]
    return expression


def notice_files(package):
    root = Path(package["manifest_path"]).parent
    files = set()
    license_file = package.get("license_file")
    if license_file:
        path = (root / license_file).resolve()
        if not path.is_file() or not path.is_relative_to(root.resolve()):
            raise ValueError(f"Invalid license-file path for {package['name']}: {license_file}")
        files.add(path)

    for directory in (root, root / "licenses", root / "license", root / "LICENSES"):
        if not directory.is_dir():
            continue
        candidates = directory.rglob("*") if directory.name == "LICENSES" else directory.iterdir()
        for path in candidates:
            if not path.is_file():
                continue
            if directory.name == "LICENSES" and path.suffix.casefold() in {".txt", ".md"}:
                files.add(path.resolve())
                continue
            stem = path.name.casefold().split(".", 1)[0]
            if any(stem == name or stem.startswith(name + "-") for name in LICENSE_NAMES):
                files.add(path.resolve())
    # Both package jobs must emit identical inventories from the same registry
    # bytes, independent of the host's path ordering and newline conventions.
    return root.resolve(), sorted(files, key=lambda path: path.relative_to(root.resolve()).as_posix())


def fetch_pinned_notice_files(package, source_root, target, output):
    """Fetch omitted crate license files only from Cargo's recorded source commit."""
    vcs_path = source_root / ".cargo_vcs_info.json"
    if not vcs_path.is_file():
        return []
    revision = json.loads(vcs_path.read_text(encoding="utf-8")).get("git", {}).get("sha1")
    repository = package.get("repository") or ""
    match = re.fullmatch(r"https://github\.com/([^/]+)/([^/]+)/?", repository)
    if not revision or not match:
        return []

    owner, repository_name = match.groups()
    vcs = json.loads(vcs_path.read_text(encoding="utf-8"))
    package_path = vcs.get("path_in_vcs", ".").strip("/")
    declared = package["license"].replace("(", " ").replace(")", " ")
    if "LicenseRef-" in declared:
        return []
    license_ids = set(re.findall(r"(?<![A-Za-z0-9.-])(?:MIT|Apache-2\.0|BSL-1\.0)(?![A-Za-z0-9.-])", declared))
    path_roots = [package_path] if package_path and package_path != "." else []
    path_roots.append("")
    wanted = set()
    for license_id in sorted(license_ids):
        wanted.update(SPDX_FILENAMES[license_id])
    wanted.update(("COPYING", "NOTICE", "COPYRIGHT"))
    found = []
    for filename in sorted(wanted, key=lambda name: (name in {"LICENSE", "COPYING", "NOTICE", "COPYRIGHT"}, name)):
        for root in path_roots:
            relative = "/".join(part for part in (root, filename) if part)
            url = f"https://raw.githubusercontent.com/{owner}/{repository_name}/{revision}/{relative}"
            try:
                request = Request(url, headers={"User-Agent": "riina-license-audit"})
                with urlopen(request, timeout=5) as response:
                    payload = response.read()
            except OSError:
                continue
            if len(payload) < 100 or b"<html" in payload[:512].lower():
                continue
            destination = target / "upstream" / (f"{Path(root).name}-{filename}" if root else filename)
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(payload)
            found.append({
                "path": str(destination.relative_to(output)).replace(os.sep, "/"),
                "source_url": url,
                "sha256": hashlib.sha256(payload).hexdigest(),
            })
    return found


def copy_sibling_notice_files(package, packages, target, output):
    """Reuse a license from another crate in the same pinned Cargo VCS tree."""
    repository = package.get("repository") or ""
    vcs_path = Path(package["manifest_path"]).parent / ".cargo_vcs_info.json"
    if not vcs_path.is_file():
        return []
    current_vcs = json.loads(vcs_path.read_text(encoding="utf-8"))
    revision = current_vcs.get("git", {}).get("sha1")
    path_in_vcs = current_vcs.get("path_in_vcs", ".").strip("/")
    match = re.fullmatch(r"https://github\.com/([^/]+)/([^/]+)/?", repository)
    if not revision or not match:
        return []

    owner, repository_name = match.groups()
    found = []
    for sibling in packages:
        if sibling["id"] == package["id"] or sibling.get("repository") != repository:
            continue
        if not sibling.get("license") or choose_license(sibling["license"]) != choose_license(
            package.get("license", "")
        ):
            continue
        sibling_root, sibling_files = notice_files(sibling)
        if not sibling_files:
            continue
        sibling_vcs_path = sibling_root / ".cargo_vcs_info.json"
        if not sibling_vcs_path.is_file():
            continue
        sibling_vcs = json.loads(sibling_vcs_path.read_text(encoding="utf-8"))
        if sibling_vcs.get("git", {}).get("sha1") != revision:
            continue
        sibling_path = sibling_vcs.get("path_in_vcs", ".").strip("/")
        for source in sibling_files:
            relative = Path(sibling_path) / source.relative_to(sibling_root)
            upstream_path = relative.as_posix().lstrip("./")
            url = (
                f"https://raw.githubusercontent.com/{owner}/{repository_name}/"
                f"{revision}/{upstream_path}"
            )
            payload = source.read_bytes()
            destination = target / "upstream" / f"{sibling['name']}-{source.name}"
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(payload)
            found.append({
                "path": str(destination.relative_to(output)).replace(os.sep, "/"),
                "source_url": url,
                "sha256": hashlib.sha256(payload).hexdigest(),
            })
    return found


def collect(metadata, output, root_name="riina", target_platform="x86_64-pc-windows-msvc"):
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    records = []
    missing = []
    packages = dependency_packages(metadata, root_name)
    for package in packages:
        if package.get("source") is None:
            continue
        expression = package.get("license")
        if not expression:
            raise ValueError(f"No declared license expression for {package['name']} {package['version']}")
        source_root, files = notice_files(package)
        vcs_path = source_root / ".cargo_vcs_info.json"
        vcs = json.loads(vcs_path.read_text(encoding="utf-8")) if vcs_path.is_file() else {}
        target = output / "packages" / f"{package['name']}-{package['version']}"
        copied = []
        for file in files:
            relative = file.relative_to(source_root)
            destination = target / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            payload = file.read_bytes()
            destination.write_bytes(payload)
            copied.append({
                "path": str(destination.relative_to(output)).replace(os.sep, "/"),
                "sha256": hashlib.sha256(payload).hexdigest(),
            })
        remote = copy_sibling_notice_files(package, packages, target, output) if not files else []
        if not files and not remote:
            remote = fetch_pinned_notice_files(package, source_root, target, output)
        if not files and not remote:
            missing.append({
                "name": package["name"],
                "version": package["version"],
                "license_expression": expression,
                "repository": package.get("repository"),
                "vcs": vcs.get("git"),
                "path_in_vcs": vcs.get("path_in_vcs", "."),
            })
        records.append({
            "name": package["name"],
            "version": package["version"],
            "license_expression": expression,
            "selected_license": choose_license(expression),
            "authors": package.get("authors", []),
            "repository": package.get("repository"),
            "vcs": vcs.get("git"),
            "path_in_vcs": vcs.get("path_in_vcs", "."),
            "license_files": copied,
            "pinned_upstream_license_files": remote,
        })

    for record in records:
        chosen = set(record["selected_license"].split(" AND "))
        unsupported = chosen - GPLV3_COMPATIBLE_GRANTS
        if unsupported:
            exception = REVIEWED_DATA_GRANTS.get((
                record["name"], record["version"], record["selected_license"]
            ))
            if exception is None or unsupported != {"CDLA-Permissive-2.0"}:
                raise ValueError(
                    f"No reviewed GPLv3-compatible grant or scoped data grant for "
                    f"{record['name']} {record['version']}: {sorted(unsupported)}"
                )
            license_names = {Path(item["path"]).name.casefold() for item in record["license_files"]}
            license_paths = [output / item["path"] for item in record["license_files"]]
            if not any(name == "license" or name.startswith("license.") for name in license_names):
                raise ValueError("webpki-roots CDLA data grant requires its original LICENSE file")
            if not any(
                path.is_file()
                and "Community Data License Agreement - Permissive - Version 2.0"
                in path.read_text(encoding="utf-8")
                for path in license_paths
            ):
                raise ValueError("webpki-roots LICENSE does not contain the reviewed CDLA text")

            notice_path = output / "packages" / "webpki-roots-1.0.9" / "CCADB-ATTRIBUTION.txt"
            notice_path.parent.mkdir(parents=True, exist_ok=True)
            notice_path.write_text(exception["attribution"] + "\n", encoding="utf-8", newline="\n")
            record["reviewed_data_license_exception"] = {
                "classification": exception["classification"],
                "rationale": exception["rationale"],
                "source": exception["source"],
                "license_url": exception["license_url"],
                "data_source": exception["data_source"],
                "required_attribution_file": str(notice_path.relative_to(output)).replace(os.sep, "/"),
            }

    manifest = {
        "target": target_platform,
        "workspace_binary": root_name,
        "combined_distribution_license": "GPL-3.0-only",
        "selected_grants_policy": (
            "Select a reviewed GPLv3-compatible software grant and preserve every package "
            "license and notice. Only exact, documented third-party data-grant exceptions "
            "may be included; these do not classify the grant as GPL-compatible software."
        ),
        "packages": records,
    }
    (output / "RUST-LICENSES.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n"
    )
    return len(records), missing


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--manifest-path", type=Path)
    parser.add_argument("--package", default="riina")
    parser.add_argument("--target", default="x86_64-pc-windows-msvc")
    args = parser.parse_args()
    command = [args.cargo, "metadata", "--locked", "--format-version", "1", "--filter-platform", args.target]
    if args.manifest_path:
        command.extend(["--manifest-path", str(args.manifest_path)])
    result = subprocess.run(
        command,
        check=True, capture_output=True, text=True, encoding="utf-8",
    )
    metadata = json.loads(result.stdout)
    count, missing = collect(metadata, args.output, args.package, args.target)
    print(f"Collected declared licenses and notices for {count} {args.package} dependencies.")
    if missing:
        print("Packages without packaged license text:", file=sys.stderr)
        print(json.dumps(missing, indent=2, ensure_ascii=False), file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"Rust license collection failed: {error}", file=sys.stderr)
        raise SystemExit(1)
