"""Assemble or verify a Windows x64 runtime without relying on the runner's PATH."""
import argparse
import json
from pathlib import Path
import shutil
import struct


# Windows 11 supplies these libraries. Keep redistributable compiler runtimes
# outside this set: a developer machine having them does not make a ZIP portable.
SYSTEM_DLLS = set("""
advapi32 avicap32 avrt bcrypt bcryptprimitives cabinet cfgmgr32 combase comctl32 comdlg32 crypt32 cryptbase
d2d1 d3d11 d3d12 d3dcompiler_47 dcomp dbghelp dnsapi dsound dwmapi dwrite
dxgi dxva2 gdi32 glu32 hid imm32 iphlpapi kernel32 mf mfplat mfreadwrite
mfuuid msacm32 msvcrt mswsock ncrypt netapi32 normaliz ntdll ole32 oleacc
oleaut32 opengl32 powrprof propsys psapi rpcrt4 secur32 setupapi shell32
shcore shlwapi strmiids ucrtbase uiautomationcore user32 userenv usp10 uxtheme version
winhttp wininet winmm winspool wintrust wldap32 ws2_32 wtsapi32
""".split())


def system_library(name):
    return name.startswith(("api-ms-win-", "ext-ms-win-")) or name.removesuffix(".dll") in SYSTEM_DLLS


def imports(path):
    """Read PE import names; delayed optional plug-ins are recorded separately."""
    data = path.read_bytes()

    def unpack(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(data):
            raise ValueError(f"Truncated PE header: {path}")
        return struct.unpack_from(fmt, data, offset)

    if data[:2] != b"MZ":
        raise ValueError(f"Not a Windows executable: {path}")
    pe, = unpack("<I", 0x3c)
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError(f"Invalid PE signature: {path}")
    machine, count = unpack("<HH", pe + 4)
    optional_size, = unpack("<H", pe + 20)
    optional = pe + 24
    magic, = unpack("<H", optional)
    if machine != 0x8664 or magic != 0x20b:
        raise ValueError(f"Expected Windows x64 PE: {path}")
    directory_count, = unpack("<I", optional + 108)
    if optional_size < 112 + min(directory_count, 14) * 8:
        raise ValueError(f"Truncated data directories: {path}")
    sections = []
    for index in range(count):
        size, address, raw_size, raw = unpack("<IIII", optional + optional_size + index * 40 + 8)
        sections.append((address, max(size, raw_size), raw, raw_size))

    def offset(rva):
        for address, size, raw, raw_size in sections:
            if address <= rva < address + size and rva - address < raw_size:
                return raw + rva - address
        raise ValueError(f"Unmapped PE address {rva:#x}: {path}")

    def name(rva):
        start = offset(rva)
        end = data.find(b"\0", start, start + 512)
        if end < 0:
            raise ValueError(f"Invalid DLL name: {path}")
        result = data[start:end].decode("ascii").lower()
        if not result.endswith(".dll") or Path(result).name != result or "/" in result or "\\" in result:
            raise ValueError(f"Unsafe DLL import name {result!r}: {path}")
        return result

    def directory(index, record_size, name_index):
        if directory_count <= index:
            return []
        rva, size = unpack("<II", optional + 112 + index * 8)
        if not rva:
            return []
        found = []
        start = offset(rva)
        for position in range(start, start + size, record_size):
            fields = unpack("<" + "I" * (record_size // 4), position)
            if not any(fields):
                return sorted(set(found))
            # PE32+ delay descriptors normally contain RVAs (attribute bit 0).
            if index == 13 and not fields[0] & 1:
                raise ValueError(f"Unsupported VA delay descriptor: {path}")
            found.append(name(fields[name_index]))
        raise ValueError(f"Unterminated PE import directory: {path}")

    return {"required": directory(1, 20, 3), "optional_delay": directory(13, 32, 1)}


def check_runtime(package, roots, search=()):
    available = {}
    for directory in search:
        for candidate in directory.glob("*"):
            if candidate.is_file() and candidate.suffix.lower() == ".dll":
                available.setdefault(candidate.name.lower(), candidate)
    pending = list(roots)
    seen = set()
    report = {}
    while pending:
        path = pending.pop()
        if path.name.lower() in seen:
            continue
        seen.add(path.name.lower())
        libraries = imports(path)
        report[path.name] = libraries
        packaged = {p.name.lower(): p for p in package.iterdir() if p.is_file() and p.suffix.lower() == ".dll"}
        for library in libraries["required"]:
            if system_library(library):
                continue
            dependency = packaged.get(library)
            if dependency is None and library in available:
                dependency = package / available[library].name
                shutil.copy2(available[library], dependency)
            if dependency is None:
                raise ValueError(f"{path.name} needs {library}, absent from the portable package and audited build outputs")
            pending.append(dependency)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--engine", type=Path)
    parser.add_argument("--search", type=Path, action="append", default=[])
    args = parser.parse_args()
    args.package.mkdir(parents=True, exist_ok=True)
    if args.engine:
        if args.engine.name != "libmpv-2.dll" or not args.engine.is_file():
            raise ValueError("Expected libmpv-2.dll from the builder's mpv-dev output")
        shutil.copy2(args.engine, args.package / args.engine.name)
    roots = [args.package / "libmpv-2.dll"]
    if (args.package / "riina.exe").is_file():
        roots.append(args.package / "riina.exe")
    report = check_runtime(args.package, roots, args.search)
    (args.package / "WINDOWS-IMPORTS.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Verified Windows x64 imports for {len(report)} packaged binaries")


if __name__ == "__main__":
    main()
