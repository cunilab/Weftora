import importlib.util
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("imports", Path(__file__).resolve().parents[1] / "audit-windows-imports.py")
imports = importlib.util.module_from_spec(spec)
spec.loader.exec_module(imports)

HEADERS = """file format coff-x86-64
Magic 020b (PE32+)
Entry 1 000000001000 00000028 Import Directory
Entry d 000000000000 00000000 Delay Import Directory
The Import Tables:
    DLL Name: KERNEL32.dll
    DLL Name: kernel32.dll
    DLL Name: VCRUNTIME140.dll
"""


def pe_bytes(tag=b""):
    data = bytearray(80)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 60, 64)
    data[64:70] = b"PE\0\0\x64\x86"
    return bytes(data) + tag


class WindowsImportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.tool = self.root / "llvm-objdump"
        self.tool.write_bytes(b"not executable; subprocess is mocked")
        self.package = self.root / "toolkit"
        self.package.mkdir()
        (self.package / "weftora.exe").write_bytes(pe_bytes(b"cli"))
        (self.package / "weftora-player.exe").write_bytes(pe_bytes(b"player"))
        (self.package / "notice.txt").write_text("terms")
        self.output = self.root / "audit"

    def command(self, command, **kwargs):
        self.assertEqual(command[0], str(self.tool))
        if command[1] == "--version":
            return subprocess.CompletedProcess(command, 0, "LLVM (http://llvm.org/):\n  LLVM version 20.1.8\n")
        self.assertEqual(command[1], "--private-headers")
        kwargs["stdout"].write(HEADERS.encode())
        return subprocess.CompletedProcess(command, 0)

    def test_imports_normalize_case_and_refuse_delay_missing_or_unsupported_output(self):
        self.assertEqual(imports.imports(HEADERS), ["kernel32.dll", "vcruntime140.dll"])
        for text in (HEADERS.replace("020b", "010b"), HEADERS.replace("Entry d", "Other d"),
                     HEADERS.replace("000000000000", "000000001000"),
                     HEADERS.replace("VCRUNTIME140.dll", "../outside.dll"),
                     HEADERS[:HEADERS.index("The Import Tables:")], HEADERS + "Entry 1 0 0 duplicate\n"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                imports.imports(text)

    def test_magic_refuses_malformed_native_files_and_accepts_plain_notices(self):
        self.assertTrue(imports.is_pe(self.package / "weftora.exe"))
        self.assertFalse(imports.is_pe(self.package / "notice.txt"))
        for data in (b"not PE", b"MZ", pe_bytes().replace(b"\x64\x86", b"\x64\xaa")):
            (self.package / "weftora.exe").write_bytes(data)
            with self.subTest(data=data), self.assertRaises(ValueError):
                imports.is_pe(self.package / "weftora.exe")

    def test_all_package_pe_files_are_recorded_with_deduplicated_inspection(self):
        game = self.root / "game"
        game.mkdir()
        (game / "renamed.data").write_bytes(pe_bytes(b"player"))
        with patch.object(imports.subprocess, "run", side_effect=self.command) as run:
            result = imports.audit([self.package, game], self.tool, self.output)
        self.assertEqual(run.call_count, 3)
        self.assertEqual(len(result["binaries"]), 2)
        self.assertEqual([len(p["peFiles"]) for p in result["packages"]], [2, 1])
        self.assertFalse(result["auditApproved"])
        self.assertTrue((self.output / "windows-imports.json").is_file())

    def test_upstream_and_apple_versions_are_recorded_and_missing_version_refuses(self):
        for index, version in enumerate(("LLVM (http://llvm.org/):\n  LLVM version 20.1.8\n", "Apple LLVM version 21.0.0\n", "LLVM without a version\n")):
            output = self.root / ("version-" + str(index))
            def command(args, **kwargs):
                if args[1] == "--version":
                    return subprocess.CompletedProcess(args, 0, version)
                return self.command(args, **kwargs)
            with patch.object(imports.subprocess, "run", side_effect=command):
                if index == 2:
                    with self.assertRaisesRegex(ValueError, "version evidence"):
                        imports.audit([self.package], self.tool, output)
                    self.assertFalse(output.exists())
                else:
                    result = imports.audit([self.package], self.tool, output)
                    self.assertEqual(result["llvmVersion"], version.splitlines()[-1].strip())

    def test_mutated_candidate_or_delayed_imports_never_get_success_report(self):
        for mutate in (False, True):
            output = self.root / str(mutate)
            def command(args, **kwargs):
                result = self.command(args, **kwargs)
                if args[1] != "--version":
                    if mutate:
                        (self.package / "notice.txt").write_text("changed")
                    else:
                        kwargs["stdout"].seek(0)
                        kwargs["stdout"].truncate()
                        kwargs["stdout"].write(HEADERS.replace("000000000000", "000000001000").encode())
                return result
            with patch.object(imports.subprocess, "run", side_effect=command):
                with self.assertRaisesRegex(ValueError, "package changed" if mutate else "delay imports"):
                    imports.audit([self.package], self.tool, output)
            self.assertFalse((output / "windows-imports.json").exists())

    def test_fresh_output_overlap_links_and_candidate_inspector_are_refused(self):
        self.output.mkdir()
        for packages, tool, output in (([self.package], self.tool, self.output),
                                       ([self.package], self.tool, self.package / "audit"),
                                       ([self.package, self.package], self.tool, self.root / "other"),
                                       ([self.package], self.package / "weftora.exe", self.root / "other")):
            with self.subTest(packages=packages, output=output), self.assertRaises(ValueError):
                imports.audit(packages, tool, output)
        link = self.package / "linked"
        try:
            link.symlink_to(self.tool)
        except OSError:
            self.skipTest("symlink creation unavailable")
        with self.assertRaises(ValueError):
            imports.audit([self.package], self.tool, self.root / "other")


if __name__ == "__main__":
    unittest.main()
