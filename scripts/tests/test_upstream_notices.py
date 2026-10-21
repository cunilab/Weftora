import hashlib
import importlib.util
import json
from pathlib import Path
import unittest
import tarfile

spec = importlib.util.spec_from_file_location("authoring", Path(__file__).resolve().parents[1] / "stage-authoring-tools.py")
authoring = importlib.util.module_from_spec(spec)
spec.loader.exec_module(authoring)


class UpstreamNoticeDataTests(unittest.TestCase):
    def test_embedded_default_font_retains_its_own_notice(self):
        root = Path(__file__).resolve().parents[2] / "third-party/notices"
        index = json.loads((root / "index.json").read_text(encoding="utf-8"))
        package = next(p for p in index["packages"] if p["name"] == "bevy_text" and p["version"] == "0.20.0")
        font = next(f for f in package["files"] if f["path"].endswith("/assets/fonts/FiraMono-LICENSE"))
        text = (root / font["path"]).read_text(encoding="utf-8")
        self.assertIn("SIL OPEN FONT LICENSE Version 1.1", text)
        self.assertIn("The Mozilla Foundation and Telefonica S.A.", text)
        self.assertEqual(font["kind"], "embedded-asset-notice")

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
                    if file["kind"] == "source-archive":
                        name = package["name"] + "-" + package["version"]
                        self.assertEqual(file["url"], f"https://static.crates.io/crates/{package['name']}/{name}.crate")
                        self.assertEqual(path.name, name + ".crate")
                        authoring.check_archive_lock(package, root.parents[1] / "Cargo.lock", file["sha256"])
                        with tarfile.open(root / path) as archive:
                            for member in archive.getmembers():
                                relative = Path(member.name)
                                self.assertFalse(relative.is_absolute())
                                self.assertNotIn("..", relative.parts)
                                self.assertEqual(relative.parts[0], name)
                                self.assertTrue(member.isfile() or member.isdir())
                            self.assertIn(name + "/src/lib.rs", archive.getnames())
                            self.assertIn(name + "/LICENCE.md", archive.getnames())
                            vcs = json.load(archive.extractfile(name + "/.cargo_vcs_info.json"))
                            self.assertEqual(vcs["git"]["sha1"], package["upstreamCommit"])
                    else:
                        self.assertRegex(file["url"], r"^https://(raw\.githubusercontent\.com|gitlab\.com)/")
                        self.assertIn("/" + package["upstreamCommit"] + "/", file["url"])
                    self.assertIn(file["kind"], ("licence-text", "licensing-policy", "embedded-asset-notice", "source-archive"))
                    self.assertEqual(hashlib.sha256((root / path).read_bytes()).hexdigest(), file["sha256"])
                    files.add(path.as_posix())
        actual = {path.relative_to(root).as_posix() for path in root.rglob("*")
                  if path.is_file() and path not in (root / "index.json", root / "README.md")}
        self.assertEqual(files, actual)


if __name__ == "__main__":
    unittest.main()
