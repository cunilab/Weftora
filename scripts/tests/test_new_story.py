import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("starter", Path(__file__).resolve().parents[1] / "verify-new-story.py")
starter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(starter)


class HelloEditTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.story = Path(self.temp.name) / "hello"
        shutil.copytree(Path(__file__).resolve().parents[2] / "examples/hello-story", self.story)

    def test_only_greeting_changes_and_repeated_edit_refuses(self):
        path = self.story / "dialogue/intro.yarn"
        original = path.read_text(encoding="utf-8")
        starter.edit_hello(self.story)
        self.assertEqual(path.read_text(encoding="utf-8"), original.replace(starter.HELLO_OLD, starter.HELLO_NEW))
        before = starter.inventory(self.story)
        with self.assertRaisesRegex(SystemExit, "baseline Hello Story greeting"):
            starter.edit_hello(self.story)
        self.assertEqual(starter.inventory(self.story), before)

    def test_missing_duplicate_or_preexisting_greeting_refuses_before_write(self):
        path = self.story / "dialogue/intro.yarn"
        for text in ("", (starter.HELLO_OLD + "\n") * 2, starter.HELLO_OLD + "\n" + starter.HELLO_NEW):
            path.write_text(text, encoding="utf-8")
            before = starter.inventory(self.story)
            with self.assertRaisesRegex(SystemExit, "baseline Hello Story greeting"):
                starter.edit_hello(self.story)
            self.assertEqual(starter.inventory(self.story), before)

    def test_optimized_cli_refuses_output_inside_checkout_before_creation(self):
        output = Path(__file__).resolve().parents[2] / ("hello-proof-refused-" + self.story.parent.name)
        self.assertFalse(output.exists())
        result = subprocess.run([sys.executable, "-B", "-O", starter.__file__, "--cli", sys.executable,
                                 "--player", sys.executable, "--out", str(output)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("output must be outside checkout", result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
