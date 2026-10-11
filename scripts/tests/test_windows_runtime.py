"""Portable-package regression cases, independent of installed Windows DLLs."""
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest


spec = importlib.util.spec_from_file_location("runtime", Path(__file__).parents[1] / "check-windows-runtime.py")
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def executable(path, required=(), delayed=(), machine=0x8664):
    data = bytearray(4096)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3c, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HH", data, 0x84, machine, 1)
    struct.pack_into("<H", data, 0x94, 240)
    struct.pack_into("<H", data, 0x98, 0x20b)
    struct.pack_into("<I", data, 0x98 + 108, 16)
    struct.pack_into("<IIII", data, 0x98 + 240 + 8, 3072, 0x1000, 3072, 0x400)
    string_at = 0x900
    for index, names, table_at, record_size, name_index in [(1, required, 0x400, 20, 3), (13, delayed, 0x600, 32, 1)]:
        if not names:
            continue
        struct.pack_into("<II", data, 0x98 + 112 + index * 8, table_at + 0xc00, (len(names) + 1) * record_size)
        for number, name in enumerate(names):
            descriptor = [0] * (record_size // 4)
            if index == 13:
                descriptor[0] = 1
            descriptor[name_index] = string_at + 0xc00
            struct.pack_into("<" + "I" * len(descriptor), data, table_at + number * record_size, *descriptor)
            encoded = name.encode("ascii") + b"\0"
            data[string_at:string_at + len(encoded)] = encoded
            string_at += len(encoded)
    path.write_bytes(data)
    return path


class WindowsRuntimeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.package = self.root / "package"
        self.package.mkdir()

    def test_required_and_optional_imports_are_distinct(self):
        path = executable(self.package / "libmpv-2.dll", ["KERNEL32.dll", "vulkan-1.dll"], ["VapourSynth.dll"])
        self.assertEqual(runtime.imports(path), {"required": ["kernel32.dll", "vulkan-1.dll"], "optional_delay": ["vapoursynth.dll"]})

    def test_copy_and_verify_transitive_dependencies_from_build_outputs(self):
        build = self.root / "bin"
        build.mkdir()
        root = executable(self.package / "libmpv-2.dll", ["VULKAN-1.DLL"])
        executable(build / "VULKAN-1.DLL", ["loader-support.dll"])
        executable(build / "loader-support.dll", ["kernel32.dll"])
        report = runtime.check_runtime(self.package, [root], [build])
        self.assertEqual(len(report), 3)
        self.assertTrue((self.package / "loader-support.dll").is_file())

    def test_missing_transitive_dependency_is_an_error(self):
        root = executable(self.package / "libmpv-2.dll", ["vulkan-1.dll"])
        executable(self.package / "vulkan-1.dll", ["missing.dll"])
        with self.assertRaisesRegex(ValueError, "vulkan-1.dll needs missing.dll"):
            runtime.check_runtime(self.package, [root])

    def test_compiler_redistributable_is_not_treated_as_windows(self):
        root = executable(self.package / "riina.exe", ["vcruntime140.dll"])
        with self.assertRaisesRegex(ValueError, "vcruntime140.dll"):
            runtime.check_runtime(self.package, [root])

    def test_wrong_architecture_and_truncated_headers_fail(self):
        root = executable(self.package / "riina.exe", machine=0x14c)
        with self.assertRaisesRegex(ValueError, "Windows x64"):
            runtime.imports(root)
        root.write_bytes(b"MZ")
        with self.assertRaisesRegex(ValueError, "Truncated PE"):
            runtime.imports(root)


if __name__ == "__main__":
    unittest.main()
