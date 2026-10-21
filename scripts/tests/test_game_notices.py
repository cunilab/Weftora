import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("game_notices", Path(__file__).resolve().parents[1] / "stage-game-notices.py")
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)


class GameNoticeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.count = 0

    def fixture(self):
        self.count += 1
        root = self.root / str(self.count)
        toolkit, game = root / "tools", root / "game"
        toolkit.mkdir(parents=True)
        game.mkdir()
        player = game / "weftora-player.exe"
        player.write_bytes(b"fixture player, not executable")
        (game / "story").mkdir()
        (game / "story/manifest.jsonc").write_text("story bytes")
        (toolkit / "weftora.exe").write_bytes(b"compiler must not be copied")
        (toolkit / "README.txt").write_text("toolkit instructions must not be copied")
        licence = toolkit / "licenses/dependency-1.0/LICENSE"
        licence.parent.mkdir(parents=True)
        licence.write_bytes(b"exact upstream terms\r\n")
        licence.chmod(0o755)
        second = licence.with_name("NOTICE")
        second.write_bytes(b"additional upstream attribution\n")
        report = {"auditApproved": False, "cargoLockSha256": notices.transport.file_hash(Path(__file__).resolve().parents[2] / "Cargo.lock"),
                  "playerIdentity": {"tool": "weftora-player", "buildId": "fixture"},
                  "cliIdentity": {"tool": "weftora", "buildId": "fixture"}, "cliSha256": "c" * 64,
                  "playerSha256": notices.transport.file_hash(player), "packages": [
                      {"name": "dependency", "missingTextOrMetadata": False, "files": [
                          {"path": "licenses/dependency-1.0/LICENSE", "sha256": notices.transport.file_hash(licence)},
                          {"path": "licenses/dependency-1.0/NOTICE", "sha256": notices.transport.file_hash(second)}]},
                      {"name": "unresolved", "missingTextOrMetadata": True, "files": []}]}
        for name in notices.ROOT_FILES:
            (toolkit / name).write_bytes(b"root terms\r\n")
        self.write_report(toolkit, report)
        export = {"exportFormat": 1, "player": {"file": player.name, "identity": report["playerIdentity"], "sha256": report["playerSha256"]},
                  "cli": {"identity": report["cliIdentity"], "sha256": report["cliSha256"]}}
        (game / "weftora-export.json").write_text(json.dumps(export))
        return toolkit, game

    def write_report(self, toolkit, report):
        (toolkit / "dependency-notices.json").write_text(json.dumps(report))

    def test_notices_preserve_bytes_missing_flags_and_game_without_copying_tools(self):
        toolkit, game = self.fixture()
        before = notices.transport.inventory(game)
        approved = notices.snapshot(toolkit)
        output = notices.stage(toolkit, game)
        self.assertEqual(notices.snapshot(output), approved)
        copied = json.loads((output / "dependency-notices.json").read_text())
        self.assertTrue(copied["packages"][1]["missingTextOrMetadata"])
        self.assertFalse(copied["auditApproved"])
        self.assertEqual((output / "licenses/dependency-1.0/LICENSE").read_bytes(), b"exact upstream terms\r\n")
        proof = json.loads((output / "notice-provenance.json").read_text())
        self.assertEqual(proof["playerSha256"], copied["playerSha256"])
        self.assertEqual(proof["toolkitReportSha256"], approved["dependency-notices.json"]["sha256"])
        after = notices.transport.inventory(game)
        self.assertEqual({key: after[key] for key in before}, before)
        self.assertEqual({path.name for path in output.rglob("*") if path.suffix == ".exe"}, set())
        self.assertFalse((output / "README.txt").exists())

    def test_mismatched_pair_lock_or_approval_refuses_before_writing(self):
        for field in ("playerSha256", "cliSha256", "playerIdentity", "cliIdentity", "cargoLockSha256", "auditApproved"):
            with self.subTest(field=field):
                toolkit, game = self.fixture()
                report = notices.read_json(toolkit / "dependency-notices.json")
                report[field] = True if field == "auditApproved" else "mismatch"
                self.write_report(toolkit, report)
                reason = {"playerSha256": "player hashes", "cargoLockSha256": "lockfile", "auditApproved": "unapproved"}.get(field, "identities differ")
                with self.assertRaisesRegex(ValueError, reason):
                    notices.stage(toolkit, game)
                self.assertFalse((game / "notices").exists())
        toolkit, game = self.fixture()
        (game / "weftora-player.exe").write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "player hashes"):
            notices.stage(toolkit, game)
        self.assertFalse((game / "notices").exists())

    def test_missing_extra_tampered_or_unsafe_licence_inventory_refuses(self):
        for change in ("missing", "extra", "tampered", "traversal", "duplicate"):
            with self.subTest(change=change):
                toolkit, game = self.fixture()
                licence = toolkit / "licenses/dependency-1.0/LICENSE"
                report = notices.read_json(toolkit / "dependency-notices.json")
                if change == "missing":
                    licence.unlink()
                elif change == "extra":
                    (licence.parent / "extra").write_bytes(b"unlisted")
                elif change == "tampered":
                    licence.write_bytes(b"changed")
                elif change == "traversal":
                    report["packages"][0]["files"][0]["path"] = "licenses/../outside"
                else:
                    report["packages"][0]["files"] *= 2
                self.write_report(toolkit, report)
                reason = {"traversal": "unsafe member name", "duplicate": "invalid/duplicate"}.get(change, "inventory/hash mismatch")
                with self.assertRaisesRegex(ValueError, reason):
                    notices.stage(toolkit, game)
                self.assertFalse((game / "notices").exists())

    def test_existing_output_and_overlap_preserve_contents(self):
        toolkit, game = self.fixture()
        output = game / "notices"
        output.mkdir()
        (output / "keep").write_bytes(b"keep")
        with self.assertRaisesRegex(ValueError, "fresh"):
            notices.stage(toolkit, game)
        self.assertEqual((output / "keep").read_bytes(), b"keep")
        for first, second in ((toolkit, toolkit), (toolkit, toolkit / "licenses"), (game / "story", game)):
            with self.assertRaisesRegex(ValueError, "overlap"):
                notices.stage(first, second)

    def test_invalid_metadata_and_limits_refuse(self):
        for content in (b"\xff", b"[]", b"{}", b"x" * (notices.MAX_DOCUMENT_BYTES + 1)):
            toolkit, game = self.fixture()
            (toolkit / "dependency-notices.json").write_bytes(content)
            with self.assertRaises((ValueError, KeyError)):
                notices.stage(toolkit, game)
            self.assertFalse((game / "notices").exists())
        toolkit, game = self.fixture()
        licence_count = len(notices.transport.inventory(toolkit / "licenses"))
        with mock.patch.object(notices.transport, "MAX_ENTRIES", licence_count), self.assertRaisesRegex(ValueError, "notice inventory exceeds bounds"):
            notices.stage(toolkit, game)
        self.assertFalse((game / "notices").exists())

    def test_post_copy_source_mutation_leaves_no_success_proof(self):
        toolkit, game = self.fixture()
        original = notices.snapshot
        calls = 0

        def mutate(folder):
            nonlocal calls
            calls += 1
            if calls == 3:
                (toolkit / "LICENSE-MIT").write_bytes(b"changed")
            return original(folder)

        with mock.patch.object(notices, "snapshot", side_effect=mutate), self.assertRaisesRegex(ValueError, "changed"):
            notices.stage(toolkit, game)
        self.assertTrue((game / "notices").is_dir())
        self.assertFalse((game / "notices/notice-provenance.json").exists())

    def test_export_format_player_name_and_non_string_path_refuse(self):
        for change in ("format", "player", "path"):
            toolkit, game = self.fixture()
            export = notices.read_json(game / "weftora-export.json")
            report = notices.read_json(toolkit / "dependency-notices.json")
            if change == "format":
                export["exportFormat"] = True
            elif change == "player":
                export["player"]["file"] = "../escape"
            else:
                report["packages"][0]["files"][0]["path"] = 1
            (game / "weftora-export.json").write_text(json.dumps(export))
            self.write_report(toolkit, report)
            reason = {"format": "export format", "player": "player filename", "path": "must be a string"}[change]
            with self.assertRaisesRegex(ValueError, reason):
                notices.stage(toolkit, game)
            self.assertFalse((game / "notices").exists())

    def test_optimized_cli_success_and_refusal(self):
        toolkit, game = self.fixture()
        command = [sys.executable, "-B", "-O", notices.__file__, "--toolkit", str(toolkit), "--game", str(game)]
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")
        self.assertIn("must be fresh", result.stderr)

    @unittest.skipIf(os.name == "nt", "Windows symlinks require privilege; junction tested separately")
    def test_linked_input_notice_or_player_is_rejected(self):
        for target in ("toolkit", "licence", "player"):
            toolkit, game = self.fixture()
            if target == "toolkit":
                link = toolkit.with_name("link")
                link.symlink_to(toolkit, target_is_directory=True)
                toolkit = link
            else:
                path = toolkit / "LICENSE-MIT" if target == "licence" else game / "weftora-player.exe"
                path.unlink()
                path.symlink_to(toolkit / "LICENSE-APACHE")
            with self.assertRaisesRegex(ValueError, "links/reparse"):
                notices.stage(toolkit, game)
            self.assertFalse((game / "notices").exists())

    @unittest.skipUnless(os.name == "nt", "native Windows junction")
    def test_windows_junction_is_rejected(self):
        toolkit, game = self.fixture()
        junction = toolkit.with_name("junction")
        subprocess.run(["cmd", "/c", "mklink", "/J", str(junction), str(toolkit)], check=True, capture_output=True)
        try:
            with self.assertRaisesRegex(ValueError, "links/reparse"):
                notices.stage(junction, game)
        finally:
            junction.rmdir()
        self.assertFalse((game / "notices").exists())


if __name__ == "__main__":
    unittest.main()
