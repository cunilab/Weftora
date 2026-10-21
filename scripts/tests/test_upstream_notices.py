import hashlib
import json
from pathlib import Path
import unittest


class UpstreamNoticeDataTests(unittest.TestCase):
    def test_checked_in_texts_match_pinned_provenance_and_have_no_orphan_files(self):
        root = Path(__file__).resolve().parents[2] / "third-party/notices"
        index = json.loads((root / "index.json").read_text(encoding="utf-8"))
        self.assertEqual(index["format"], 1)
        packages, files = set(), set()
        for package in index["packages"]:
            key = (package["name"], package["version"])
            self.assertNotIn(key, packages)
            packages.add(key)
            self.assertRegex(package["upstreamCommit"], r"^[0-9a-f]{40}$")
            self.assertTrue(package["files"])
            for file in package["files"]:
                with self.subTest(package=key, file=file["path"]):
                    path = Path(file["path"])
                    self.assertFalse(path.is_absolute())
                    self.assertNotIn("..", path.parts)
                    self.assertFalse((root / path).is_symlink())
                    self.assertTrue((root / path).resolve().is_relative_to(root.resolve()))
                    self.assertRegex(file["url"], r"^https://(raw\.githubusercontent\.com|gitlab\.com)/")
                    self.assertIn("/" + package["upstreamCommit"] + "/", file["url"])
                    self.assertIn(file["kind"], ("licence-text", "licensing-policy"))
                    self.assertEqual(hashlib.sha256((root / path).read_bytes()).hexdigest(), file["sha256"])
                    files.add(path.as_posix())
        actual = {path.relative_to(root).as_posix() for path in root.rglob("*")
                  if path.is_file() and path not in (root / "index.json", root / "README.md")}
        self.assertEqual(files, actual)


if __name__ == "__main__":
    unittest.main()
