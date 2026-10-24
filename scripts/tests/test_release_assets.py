import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("release_assets", Path(__file__).resolve().parents[1] / "stage-release-assets.py")
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)


class ReleaseAssetTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.packages = []
        for name in ("tools", "story-a", "life-sim", "localization-demo"):
            source = self.root / name
            source.mkdir()
            (source / "content.txt").write_text(name, encoding="utf-8")
            output = self.root / (name + "-transport")
            assets.transport.package(source, output)
            self.packages.append(output)
        self.output = self.root / "release-assets"

    def test_four_real_packages_stage_exact_bytes_and_deterministic_combined_checksums(self):
        assets.stage(self.packages, self.output)
        inventory = json.loads((self.output / "release-assets.json").read_text())
        self.assertEqual([row["name"] for row in inventory], ["life-sim.zip", "localization-demo.zip", "story-a.zip", "tools.zip"])
        expected = ""
        for row in inventory:
            source = self.root / row["sourcePackage"] / row["name"]
            staged = self.output / row["name"]
            self.assertEqual(source.read_bytes(), staged.read_bytes())
            digest = hashlib.sha256(staged.read_bytes()).hexdigest()
            self.assertEqual(row["sha256"], digest)
            self.assertEqual(row["size"], staged.stat().st_size)
            expected += f'{digest}  {row["name"]}\n'
        self.assertEqual((self.output / "SHA256SUMS").read_bytes(), expected.encode())
        second = self.root / "second"
        assets.stage(list(reversed(self.packages)), second)
        for name in ("SHA256SUMS", "release-assets.json"):
            self.assertEqual((self.output / name).read_bytes(), (second / name).read_bytes())

    def test_optimized_cli_stages_four_packages_and_refuses_tampering(self):
        command = [sys.executable, "-B", "-O", str(Path(assets.__file__))]
        for package in self.packages:
            command.extend(["--package", str(package)])
        result = subprocess.run(command + ["--out", str(self.output)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len((self.output / "SHA256SUMS").read_text().splitlines()), 4)
        archive = self.packages[0] / "tools.zip"
        archive.write_bytes(archive.read_bytes() + b"tampered")
        refused = self.root / "refused"
        result = subprocess.run(command + ["--out", str(refused)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum mismatch", result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertFalse(refused.exists())

    def test_missing_invalid_utf8_or_linked_checksum_is_rejected(self):
        sums = self.packages[0] / "SHA256SUMS"
        sums.unlink()
        with self.assertRaises(OSError):
            assets.stage(self.packages, self.output)
        sums.write_bytes(b"\xff")
        with self.assertRaises(ValueError):
            assets.stage(self.packages, self.output)
        if os.name != "nt":
            sums.unlink()
            sums.symlink_to(self.packages[1] / "SHA256SUMS")
            with self.assertRaisesRegex(ValueError, "links/reparse"):
                assets.stage(self.packages, self.output)
        self.assertFalse(self.output.exists())

    def test_tampered_zip_refuses_before_output_creation(self):
        archive = self.packages[0] / "tools.zip"
        archive.write_bytes(archive.read_bytes() + b"changed")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            assets.stage(self.packages, self.output)
        self.assertFalse(self.output.exists())

    def test_output_inside_package_is_rejected_without_modifying_input(self):
        package = self.packages[0]
        before = sorted(path.name for path in package.iterdir())
        output = package / "downloads"
        with self.assertRaisesRegex(ValueError, "package/output overlap"):
            assets.stage(self.packages, output)
        self.assertFalse(output.exists())
        self.assertEqual(sorted(path.name for path in package.iterdir()), before)

    def test_missing_extra_or_linked_zip_and_linked_input_are_rejected(self):
        archive = self.packages[0] / "tools.zip"
        saved = archive.read_bytes()
        archive.unlink()
        with self.assertRaisesRegex(ValueError, "exactly the listed ZIP"):
            assets.stage(self.packages, self.output)
        archive.write_bytes(saved)
        extra = self.packages[0] / "extra.ZIP"
        extra.write_bytes(saved)
        with self.assertRaisesRegex(ValueError, "exactly the listed ZIP"):
            assets.stage(self.packages, self.output)
        extra.unlink()
        if os.name != "nt":
            link = self.root / "linked-package"
            link.symlink_to(self.packages[0], target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "links/reparse"):
                assets.stage([link], self.output)
            archive.unlink()
            archive.symlink_to(self.packages[1] / "story-a.zip")
            with self.assertRaisesRegex(ValueError, "links/reparse"):
                assets.stage(self.packages, self.output)
        self.assertFalse(self.output.exists())

    def test_bad_checksum_manifests_and_unsafe_names_are_rejected(self):
        sums = self.packages[0] / "SHA256SUMS"
        digest = hashlib.sha256((self.packages[0] / "tools.zip").read_bytes()).hexdigest()
        for content in ["", "bad", f"{digest}  tools.zip\n{digest}  tools.zip\n",
                        f"{digest}  ../escape.zip\n", f"{digest}  CON.zip\n",
                        f"{digest}  end.zip.\n", f"{digest}  sub/file.zip\n", "x" * (assets.MAX_SUMS_BYTES + 1)]:
            with self.subTest(content=content[:80]):
                sums.write_text(content, encoding="utf-8")
                with self.assertRaises(ValueError):
                    assets.stage(self.packages, self.output)
                self.assertFalse(self.output.exists())

    def test_duplicate_case_and_unicode_aliases_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "aliased"):
            assets.stage([self.packages[0], self.packages[0]], self.output)
        for names in [("Name.zip", "name.zip"), ("é.zip", "e\u0301.zip")]:
            for package, old, new in zip(self.packages[:2], ["tools.zip", "story-a.zip"], names):
                old_file = next(package.glob("*.zip"))
                target = package / new
                old_file.rename(target)
                digest = hashlib.sha256(target.read_bytes()).hexdigest()
                (package / "SHA256SUMS").write_text(f"{digest}  {new}\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "aliased"):
                assets.stage(self.packages[:2], self.output)
            self.assertFalse(self.output.exists())

    def test_count_byte_limits_and_existing_output_preserve_contents(self):
        for packages in [[], self.packages * assets.MAX_PACKAGES]:
            with self.assertRaisesRegex(ValueError, "between 1 and 16"):
                assets.stage(packages, self.output)
        with mock.patch.object(assets, "MAX_ASSET_BYTES", 1), self.assertRaisesRegex(ValueError, "size/type"):
            assets.stage(self.packages, self.output)
        with mock.patch.object(assets, "MAX_TOTAL_BYTES", 1), self.assertRaisesRegex(ValueError, "total"):
            assets.stage(self.packages, self.output)
        self.assertFalse(self.output.exists())
        self.output.mkdir()
        keep = self.output / "keep"
        keep.write_bytes(b"preserve")
        with self.assertRaisesRegex(ValueError, "must be fresh"):
            assets.stage(self.packages, self.output)
        self.assertEqual(keep.read_bytes(), b"preserve")

    def test_source_or_staged_mutation_never_emits_success_manifest(self):
        original = assets.verify_source
        for change in ["source", "staged", "sums"]:
            output = self.root / ("changed-" + change)
            source = self.packages[0] / "tools.zip"
            sums = self.packages[0] / "SHA256SUMS"
            archive_bytes, sums_bytes = source.read_bytes(), sums.read_bytes()
            calls = 0

            def mutate(record):
                nonlocal calls
                calls += 1
                if calls == 2:
                    path = {"source": source, "staged": output / "tools.zip", "sums": sums}[change]
                    path.write_bytes(path.read_bytes() + b"changed")
                original(record)

            with mock.patch.object(assets, "verify_source", side_effect=mutate), self.assertRaises(ValueError):
                assets.stage([self.packages[0]], output)
            self.assertTrue(output.is_dir())
            self.assertFalse((output / "SHA256SUMS").exists())
            self.assertFalse((output / "release-assets.json").exists())
            source.write_bytes(archive_bytes)
            sums.write_bytes(sums_bytes)

    def test_download_verification_cli_checks_without_modifying_inputs(self):
        assets.stage(self.packages, self.output)
        before = {p.name: p.read_bytes() for p in self.output.iterdir()}
        records = assets.verify(self.output)
        self.assertEqual(len(records), 4)
        command = [sys.executable, "-B", "-O", str(Path(assets.__file__)), "--verify", str(self.output)]
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("approval/authenticity not established", result.stdout)
        self.assertEqual(before, {p.name: p.read_bytes() for p in self.output.iterdir()})
        refused = self.root / "never-created"
        result = subprocess.run(command + ["--out", str(refused)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(refused.exists())
        archive = self.output / "tools.zip"
        data = archive.read_bytes()
        archive.write_bytes(bytes([data[0] ^ 1]) + data[1:])
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum mismatch", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_download_tampering_extra_missing_and_linked_entries_refuse(self):
        for change in ("zip", "sums", "missing", "extra", "linked"):
            with self.subTest(change=change):
                output = self.root / ("verify-" + change)
                assets.stage(self.packages, output)
                archive = output / "tools.zip"
                if change == "zip":
                    data = archive.read_bytes()
                    archive.write_bytes(bytes([data[0] ^ 1]) + data[1:])
                elif change == "sums":
                    (output / "SHA256SUMS").write_bytes(b"0" * 64 + b"  tools.zip\n")
                elif change == "missing":
                    archive.unlink()
                elif change == "extra":
                    (output / "unexpected.txt").write_text("not a release asset")
                elif os.name != "nt":
                    archive.unlink()
                    archive.symlink_to(self.packages[0] / "tools.zip")
                else:
                    continue
                with self.assertRaises((ValueError, OSError)):
                    assets.verify(output)

    def test_download_inventory_rejects_invalid_shapes_aliases_paths_and_sizes(self):
        assets.stage(self.packages, self.output)
        path = self.output / "release-assets.json"
        baseline = json.loads(path.read_text())
        changes = [(None, "invalid release asset inventory"), ([], "invalid release asset inventory"),
                   ({}, "invalid release asset inventory"), ([None], "invalid release asset record"),
                   ([dict(baseline[0], size=True)], "invalid release asset size"),
                   ([dict(baseline[0], size=assets.MAX_ASSET_BYTES + 1)], "invalid release asset size"),
                   ([dict(baseline[0], name="../outside.zip")], "unsafe member name"),
                   ([dict(baseline[0], name="line\nbreak.zip")], "unsafe member name"),
                   ([dict(baseline[0], sha256="wrong")], "invalid release asset checksum"),
                   ([dict(baseline[0], sourcePackage=None)], "invalid source package label"),
                   ([baseline[0], dict(baseline[0], name=baseline[0]["name"].upper().replace(".ZIP", ".zip"))], "aliased"),
                   ([dict(baseline[0], name="café.zip"), dict(baseline[0], name="cafe\u0301.zip")], "aliased"),
                   ([baseline[0]] * (assets.MAX_PACKAGES + 1), "invalid release asset inventory"),
                   ([dict(baseline[0], name=f"large-{i}.zip", size=assets.MAX_ASSET_BYTES) for i in range(3)], "total release asset bytes"),
                   (list(reversed(baseline)), "must be sorted")]
        for records, message in changes:
            with self.subTest(records=records):
                path.write_text(json.dumps(records))
                with self.assertRaisesRegex(ValueError, message):
                    assets.verify(self.output)
        for data in (b"{broken", b"\xff"):
            path.write_bytes(data)
            with self.assertRaises(ValueError):
                assets.verify(self.output)
        path.write_bytes(b"x" * (128 * 1024 + 1))
        with self.assertRaisesRegex(ValueError, "metadata size/type"):
            assets.verify(self.output)

    def test_download_mutation_during_hashing_never_passes(self):
        original = assets.transport.file_hash
        for change in ("zip", "SHA256SUMS", "release-assets.json"):
            output = self.root / ("during-" + change)
            assets.stage(self.packages, output)
            changed = False
            def mutate(path, size):
                nonlocal changed
                digest = original(path, size)
                if not changed:
                    changed = True
                    target = path if change == "zip" else output / change
                    target.write_bytes(target.read_bytes() + b" ")
                return digest
            with mock.patch.object(assets.transport, "file_hash", side_effect=mutate), self.assertRaisesRegex(ValueError, "changed during verification"):
                assets.verify(output)

    @unittest.skipUnless(os.name == "nt", "native Windows junction")
    def test_windows_junction_input_is_rejected(self):
        junction = self.root / "junction"
        subprocess.run(["cmd", "/c", "mklink", "/J", str(junction), str(self.packages[0])], check=True, capture_output=True)
        try:
            with self.assertRaisesRegex(ValueError, "links/reparse"):
                assets.stage([junction], self.output)
            with self.assertRaisesRegex(ValueError, "links/reparse"):
                assets.verify(junction)
        finally:
            junction.rmdir()
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
