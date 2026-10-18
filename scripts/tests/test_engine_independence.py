import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("independence", Path(__file__).resolve().parents[1] / "verify-engine-independence.py")
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)


class CopyTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "repository"
        self.root.mkdir()
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)
        for name in ["Cargo.lock", "vendor/runtime.rs", "crates/core.rs", "examples/story-a/main.yarn",
                     "examples/life-sim/main.yarn", "examples/story-a-not-removed/main.yarn", "examples/hello-story/main.yarn"]:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name, encoding="utf-8")
        subprocess.run(["git", "add", "."], cwd=self.root, check=True)
        (self.root / "crates/core.rs").write_text("working tree edit", encoding="utf-8")
        (self.root / "new.rs").write_text("untracked source", encoding="utf-8")
        (self.root / ".gitignore").write_text("/target\n", encoding="utf-8")
        (self.root / "target").mkdir()
        (self.root / "target/cache").write_text("do not copy", encoding="utf-8")
        self.output = Path(self.temp.name) / "copy"

    def test_only_exact_story_folders_are_removed_and_originals_stay_unchanged(self):
        before = proof.inventory(self.root)
        copied = proof.copy_workspace(self.root, self.output, before)
        self.assertEqual(before, proof.inventory(self.root))
        for story in proof.STORIES:
            self.assertTrue((self.root / story / "main.yarn").is_file())
            self.assertFalse((self.output / story).exists())
        self.assertTrue((self.output / "examples/story-a-not-removed/main.yarn").is_file())
        self.assertTrue((self.output / "examples/hello-story/main.yarn").is_file())
        self.assertEqual((self.output / "crates/core.rs").read_text(), "working tree edit")
        self.assertEqual((self.output / "new.rs").read_text(), "untracked source")
        self.assertNotIn("target/cache", copied)

    def test_changed_source_during_copy_fails_closed(self):
        records = proof.inventory(self.root)
        (self.root / "crates/core.rs").write_text("changed after inventory", encoding="utf-8")
        with self.assertRaisesRegex(SystemExit, "source changed while copying"):
            proof.copy_workspace(self.root, self.output, records)

    def test_incomplete_dependency_source_is_rejected(self):
        records = proof.inventory(self.root)
        records.pop("Cargo.lock")
        with self.assertRaisesRegex(SystemExit, "Cargo.lock and vendored runtime"):
            proof.copy_workspace(self.root, self.output, records)

    @unittest.skipIf(sys.platform == "win32", "Windows symlink creation requires a host privilege; proof always rejects symlinks")
    def test_symlink_cannot_smuggle_content_into_the_copy(self):
        (self.root / "linked").symlink_to(self.root / "examples/story-a/main.yarn")
        with self.assertRaisesRegex(SystemExit, "source symlink"):
            proof.inventory(self.root)


class ProofGuardTests(unittest.TestCase):
    def test_unrelated_failure_cannot_hide_behind_another_missing_fixture(self):
        text = "test first ... FAILED\ntest second ... FAILED\n\n---- first stdout ----\ncannot access Story root\n\n---- second stdout ----\nunrelated arithmetic failure\n"
        with self.assertRaisesRegex(SystemExit, "unrelated reason: second"):
            proof.check_control(text, ["first", "second"])
        proof.check_control(text.replace("unrelated arithmetic failure", "cannot access Story root"), ["first", "second"])

    def test_windows_diagnostic_path_accepts_extended_prefix_case_and_separators(self):
        from unittest.mock import patch
        with patch.object(proof.os, "name", "nt"):
            self.assertTrue(proof.message_has_path(r"Story root `\\?\C:\PROOF\standalone\story`", "c:/proof/standalone/story"))
            self.assertFalse(proof.message_has_path(r"Story root `C:\other\story`", "c:/proof/standalone/story"))


if __name__ == "__main__":
    unittest.main()
