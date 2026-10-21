import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("mechanics", Path(__file__).resolve().parents[1] / "verify-mechanics-edits.py")
mechanics = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mechanics)
REPOSITORY = Path(__file__).resolve().parents[2]


class MechanicsEditTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def copy(self, name):
        target = self.root / name
        shutil.copytree(REPOSITORY / "examples" / (name + "-demo"), target)
        return target

    def test_all_guarded_edits_change_only_mechanics_and_schema(self):
        for name in mechanics.CASES:
            with self.subTest(case=name):
                target = self.copy(name)
                original = (target / "main.yarn").read_bytes()
                mechanics.edit_story(name, target)
                manifest = json.loads((target / "manifest.jsonc").read_text())
                self.assertEqual(manifest["saveSchema"], 2)
                self.assertEqual((target / "main.yarn").read_bytes(), original)
                if name == "random":
                    self.assertEqual(manifest["state"]["rng"]["default"], 1)
                elif name == "scripts":
                    self.assertIn("let limit = 3;", (target / "mechanics.rhai").read_text())
                else:
                    events = json.loads((target / "events.jsonc").read_text())
                    if name == "rules":
                        self.assertEqual(events["events"][0]["actions"][0]["amount"], 20)
                    elif name == "commands":
                        self.assertEqual(events["commands"][1]["actions"][0]["amount"], 4)
                    else:
                        actions = events["commands"][0]["actions"]
                        self.assertEqual(actions[0]["value"]["literal"], ["key", "ticket", "pass"])
                        if name == "collections":
                            self.assertEqual(actions[2]["value"], 3)
                        else:
                            self.assertNotIn("item_count", manifest["state"])
                before = mechanics.inventory(target)
                with self.assertRaisesRegex(ValueError, "baseline save schema"):
                    mechanics.edit_story(name, target)
                self.assertEqual(mechanics.inventory(target), before)

    def test_script_missing_or_duplicate_anchor_refuses_without_changes(self):
        for count in (0, 2):
            target = self.root / str(count)
            shutil.copytree(REPOSITORY / "examples/scripts-demo", target)
            path = target / "mechanics.rhai"
            path.write_text("let limit = amount;\n" * count)
            before = mechanics.inventory(target)
            with self.assertRaisesRegex(ValueError, "baseline script limit"):
                mechanics.edit_story("scripts", target)
            self.assertEqual(mechanics.inventory(target), before)

    def test_changed_rule_and_collection_baselines_refuse_without_writes(self):
        for name in ("rules", "collections", "queries", "commands", "random"):
            target = self.copy(name)
            path = target / ("manifest.jsonc" if name == "random" else "events.jsonc")
            doc = json.loads(path.read_text())
            if name == "rules":
                doc["events"][0]["actions"][0]["amount"] = 11
            elif name == "commands":
                doc["commands"][1]["actions"][0]["amount"] = 1
            elif name == "random":
                doc["state"]["rng"]["default"] = 5
            else:
                doc["commands"][0]["actions"][0]["value"]["literal"] = []
            path.write_text(json.dumps(doc))
            before = mechanics.inventory(target)
            with self.assertRaisesRegex(ValueError, "review baseline"):
                mechanics.edit_story(name, target)
            self.assertEqual(mechanics.inventory(target), before)

    def test_optimized_cli_refuses_existing_output_without_overwrite(self):
        keep = self.root / "keep"
        keep.write_bytes(b"preserve")
        result = subprocess.run([sys.executable, "-B", "-O", mechanics.__file__, "--cli", sys.executable,
                                 "--player", sys.executable, "--out", str(self.root)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("output must be fresh", result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual(keep.read_bytes(), b"preserve")

    def test_collection_and_query_mirror_guards_refuse_before_writing(self):
        for name in ("collections", "queries"):
            target = self.copy(name)
            path = target / ("events.jsonc" if name == "collections" else "manifest.jsonc")
            doc = json.loads(path.read_text())
            if name == "collections":
                doc["commands"][0]["actions"][2]["value"] = 9
            else:
                doc["state"]["item_count"] = {"default": 0}
            path.write_text(json.dumps(doc))
            before = mechanics.inventory(target)
            reason = "collection mirror" if name == "collections" else "must not have a count mirror"
            with self.assertRaisesRegex(ValueError, reason):
                mechanics.edit_story(name, target)
            self.assertEqual(mechanics.inventory(target), before)

    def test_player_in_cli_role_refuses_before_creating_output(self):
        output = self.root / "fresh"
        identity = json.dumps({"tool": "weftora-player", "buildId": "fixture"})
        result = subprocess.CompletedProcess([], 0, stdout=identity, stderr="")
        with mock.patch.object(mechanics.subprocess, "run", return_value=result), self.assertRaisesRegex(ValueError, "identities differ"):
            mechanics.prove(Path(sys.executable), Path(sys.executable), output)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
