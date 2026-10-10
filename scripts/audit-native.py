"""Record the pinned Windows DLL's own build facts; this is not a license grant."""
import argparse
import ctypes
import hashlib
import json
from pathlib import Path
import re


def inspect(dll_path, expected_hash):
    binary = dll_path.read_bytes()
    actual_hash = hashlib.sha256(binary).hexdigest()
    if actual_hash != expected_hash:
        raise ValueError("Native DLL does not match its pinned hash; refusing to load it")
    api = ctypes.CDLL(str(dll_path))
    api.mpv_create.argtypes = []
    api.mpv_create.restype = ctypes.c_void_p
    api.mpv_set_option_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
    api.mpv_set_option_string.restype = ctypes.c_int
    api.mpv_initialize.argtypes = [ctypes.c_void_p]
    api.mpv_initialize.restype = ctypes.c_int
    api.mpv_get_property_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
    # Keep the allocated pointer rather than converting it automatically: mpv_free
    # must receive that exact pointer after the bytes have been copied.
    api.mpv_get_property_string.restype = ctypes.c_void_p
    api.mpv_free.argtypes = [ctypes.c_void_p]
    api.mpv_free.restype = None
    api.mpv_terminate_destroy.argtypes = [ctypes.c_void_p]
    api.mpv_terminate_destroy.restype = None
    handle = api.mpv_create()
    if not handle:
        raise RuntimeError("mpv_create failed")
    properties = {}
    try:
        for name, value in [(b"config", b"no"), (b"load-scripts", b"no"),
                            (b"terminal", b"no"), (b"vo", b"null"), (b"ao", b"null")]:
            if api.mpv_set_option_string(handle, name, value) < 0:
                raise RuntimeError(f"Audit option rejected: {name.decode()}")
        if api.mpv_initialize(handle) < 0:
            raise RuntimeError("mpv_initialize failed")
        for name in ["mpv-version", "mpv-configuration", "ffmpeg-version"]:
            pointer = api.mpv_get_property_string(handle, name.encode("ascii"))
            if not pointer:
                raise RuntimeError(f"Required audit property unavailable: {name}")
            try:
                properties[name] = ctypes.string_at(pointer).decode("utf-8")
            finally:
                api.mpv_free(pointer)
    finally:
        api.mpv_terminate_destroy(handle)
    # Static codec dependencies do not necessarily export their configuration
    # APIs. Preserve matching embedded strings as evidence, not as an exhaustive
    # list of linked libraries or proof of corresponding-source completeness.
    configurations = sorted({
        match.group().decode("ascii")
        for match in re.finditer(rb"[\x20-\x7e]{32,}", binary)
        if b"--enable-gpl" in match.group() and b"--enable-lib" in match.group()
    })
    return {
        "dll_sha256": actual_hash,
        "runtime_properties": properties,
        "embedded_gpl_configuration_strings": configurations,
        "redistribution_approved": False,
        "limits": [
            "Embedded strings do not enumerate every linked dependency",
            "Build recipes and corresponding source still require independent verification",
            "Runtime inspection does not certify distribution license compliance",
        ],
    }


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=root / "tools/native-audit/runtime.json")
    args = parser.parse_args()
    lock = json.loads((root / "reference/native-lock.json").read_text(encoding="utf-8"))
    report = inspect(root / "tools/mpv/libmpv-2.dll", lock["dll_sha256"])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Pinned DLL inspected; {len(report['embedded_gpl_configuration_strings'])} "
          "embedded GPL codec configurations recorded. Redistribution remains unapproved.")


if __name__ == "__main__":
    main()
