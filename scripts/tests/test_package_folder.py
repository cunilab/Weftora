import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
import warnings
from unittest.mock import patch
import zipfile

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("transport", Path(__file__).resolve().parents[1] / "package-folder.py")
transport = importlib.util.module_from_spec(spec)
spec.loader.exec_module(transport)


class TransportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "game"
        (self.source / "story" / "empty").mkdir(parents=True)
        (self.source / "story" / "café.txt").write_text("Unicode story\n", encoding="utf-8")
        (self.source / "player.exe").write_bytes(b"MZ preview fixture")
        self.approved = transport.inventory(self.source)
        self.archive = self.root / "test.zip"
        self.destination = self.root / "extracted"
        self.output = self.root / "output"

    def write_archive(self, mutate=None):
        transport.write_zip(self.source, self.archive, self.approved)
        if mutate:
            with zipfile.ZipFile(self.archive) as archive:
                members = [(entry, archive.read(entry)) for entry in archive.infolist()]
            members = mutate(members)
            with warnings.catch_warnings():
                warnings.filterwarnings("ignore", message="Duplicate name:")
                with zipfile.ZipFile(self.archive, "w") as archive:
                    for entry, data in members:
                        archive.writestr(entry, data)

    def test_package_round_trip_retains_empty_directories_and_hashes_without_source_changes(self):
        (self.source / "player.exe").chmod(0o755)
        before = transport.inventory(self.source)
        archive = transport.package(self.source, self.output)
        self.assertEqual(transport.inventory(self.output / "verified" / "game"), before)
        self.assertEqual(transport.inventory(self.source), before)
        self.assertEqual((self.output / "SHA256SUMS").read_text(), f"{transport.file_hash(archive)}  game.zip\n")
        report = json.loads((self.output / "transport-proof.json").read_text())
        self.assertTrue(report["roundTripVerified"])
        self.assertEqual(report["entries"], len(before))
        self.assertEqual(json.loads((self.output / "folder-inventory.json").read_text()), before)

    def test_existing_output_and_nested_output_refuse_without_changes(self):
        self.output.mkdir()
        (self.output / "keep").write_bytes(b"unchanged")
        with self.assertRaisesRegex(ValueError, "must be fresh"):
            transport.package(self.source, self.output)
        self.assertEqual((self.output / "keep").read_bytes(), b"unchanged")
        with self.assertRaisesRegex(ValueError, "overlap"):
            transport.package(self.source, self.source / "output")
        self.assertFalse((self.source / "output").exists())

    def test_unsafe_names_are_rejected(self):
        for name in ["/absolute", "../escape", "game/../escape", "C:/escape", "game/a:b", "game\\escape",
                     "game/CON.txt", "game/CONIN$", "game/CONOUT$.log", "game/NUL .txt", "game/COM¹", "game/Lpt9.txt", "game/end.", "game/end ", "game//file",
                     "game/./file", "game/nul\x00tail", "game/line\nfeed"]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                transport.safe_name(name)

    def test_unsafe_or_unexpected_zip_entries_are_rejected_before_any_extraction(self):
        self.write_archive()
        with zipfile.ZipFile(self.archive) as archive:
            baseline = [(entry, archive.read(entry)) for entry in archive.infolist()]
        for name in ["../escape", "/escape", "C:/escape", "game\\escape", "game/extra.txt", "game/CON.txt"]:
            with self.subTest(name=name):
                entry = zipfile.ZipInfo(name)
                with zipfile.ZipFile(self.archive, "w") as archive:
                    for old, data in baseline:
                        archive.writestr(old, data)
                    archive.writestr(entry, b"unsafe")
                with self.assertRaises(ValueError):
                    transport.verify_zip(self.archive, self.destination, self.approved)
                self.assertFalse(self.destination.exists())
        self.assertFalse((self.root / "escape").exists())

    def test_duplicate_missing_and_link_members_are_rejected_before_extraction(self):
        def duplicate(members):
            return members + members[-1:]

        def link(members):
            entry, data = members[-1]
            entry.external_attr = (stat.S_IFLNK | 0o777) << 16
            return members

        for mutate in [duplicate, lambda members: members[:-1], link]:
            with self.subTest(mutate=mutate):
                self.write_archive(mutate)
                with self.assertRaises(ValueError):
                    transport.verify_zip(self.archive, self.destination, self.approved)
                self.assertFalse(self.destination.exists())
                self.archive.unlink()

    def test_changed_content_never_produces_success_report_or_checksums(self):
        original = transport.write_zip

        def mutate_then_write(folder, archive, approved):
            (folder / "player.exe").write_bytes(b"MZ changed fixture")  # Same length, different hash.
            original(folder, archive, approved)

        with patch.object(transport, "write_zip", side_effect=mutate_then_write):
            with self.assertRaisesRegex(ValueError, "content mismatch"):
                transport.package(self.source, self.output)
        self.assertFalse((self.output / "transport-proof.json").exists())
        self.assertFalse((self.output / "SHA256SUMS").exists())

    def test_size_and_count_limits_refuse_source_and_archive(self):
        with patch.object(transport, "MAX_ENTRIES", 1):
            with self.assertRaisesRegex(ValueError, "entry count"):
                transport.inventory(self.source)
        with patch.object(transport, "MAX_BYTES", 1):
            with self.assertRaisesRegex(ValueError, "uncompressed bytes"):
                transport.inventory(self.source)
        self.write_archive()
        with patch.object(transport, "MAX_ENTRIES", len(self.approved)):
            with zipfile.ZipFile(self.archive, "a") as archive:
                archive.writestr("game/extra", b"")
            with self.assertRaisesRegex(ValueError, "ZIP entry count"):
                transport.verify_zip(self.archive, self.destination, self.approved)
        with patch.object(transport, "MAX_BYTES", 1):
            with self.assertRaisesRegex(ValueError, "approved bytes"):
                transport.verify_zip(self.archive, self.destination, self.approved)
        self.assertFalse(self.destination.exists())

    def test_declared_zip_size_must_match_approved_size(self):
        self.write_archive()
        approved = {name: dict(record) for name, record in self.approved.items()}
        approved["game/player.exe"]["size"] -= 1
        with self.assertRaisesRegex(ValueError, "size mismatch"):
            transport.verify_zip(self.archive, self.destination, approved)
        self.assertFalse(self.destination.exists())

    @unittest.skipIf(os.name == "nt", "Windows symlinks need host privilege; junction coverage is separate")
    def test_links_case_aliases_and_special_files_in_source_are_rejected(self):
        link = self.source / "link"
        for target in [self.source / "player.exe", self.source / "story"]:
            link.symlink_to(target)
            with self.assertRaisesRegex(ValueError, "links/reparse"):
                transport.inventory(self.source)
            link.unlink()
        os.mkfifo(link)
        with self.assertRaisesRegex(ValueError, "regular file/directory"):
            transport.inventory(self.source)
        link.unlink()
        alias = self.source / "PLAYER.EXE"
        # Only case-sensitive hosts can hold the conflicting pair.
        if not alias.exists():
            alias.write_bytes(b"alias")
            with self.assertRaisesRegex(ValueError, "case-colliding"):
                transport.inventory(self.source)

    def test_case_aliases_are_rejected_on_every_host(self):
        alias = self.source / "PLAYER.EXE"
        if not alias.exists():
            alias.write_bytes(b"alias")
        for names in [("player.exe", "PLAYER.EXE"), ("café.txt", "cafe\u0301.txt")]:
            for name in names:
                path = self.source / name
                if not path.exists():
                    path.write_bytes(b"alias")
            with self.subTest(names=names), patch.object(transport.os, "walk", return_value=[(str(self.source), [], list(names))]):
                with self.assertRaisesRegex(ValueError, "case-colliding"):
                    transport.inventory(self.source)

    @unittest.skipUnless(os.name == "nt", "native Windows junction test")
    def test_windows_junction_and_dangling_junction_are_rejected(self):
        target = self.root / "junction-target"
        target.mkdir()
        link = self.source / "junction"
        subprocess.run(["cmd", "/c", "mklink", "/J", str(link), str(target)], check=True, capture_output=True)
        self.addCleanup(lambda: os.rmdir(link) if os.path.lexists(link) else None)
        with self.assertRaisesRegex(ValueError, "links/reparse"):
            transport.inventory(self.source)
        target.rmdir()
        with self.assertRaisesRegex(ValueError, "links/reparse"):
            transport.inventory(self.source)

    def test_existing_verification_destination_is_never_changed(self):
        self.write_archive()
        self.destination.mkdir()
        (self.destination / "keep").write_bytes(b"keep")
        with self.assertRaisesRegex(ValueError, "must be fresh"):
            transport.verify_zip(self.archive, self.destination, self.approved)
        self.assertEqual(list(self.destination.iterdir()), [self.destination / "keep"])


if __name__ == "__main__":
    unittest.main()
