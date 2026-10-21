import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("authoring", Path(__file__).resolve().parents[1] / "stage-authoring-tools.py")
authoring = importlib.util.module_from_spec(spec)
spec.loader.exec_module(authoring)


class AuthoringTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repository = self.root / "repository"
        self.repository.mkdir()
        for name in ("LICENSE-MIT", "LICENSE-APACHE"):
            (self.repository / name).write_text(name)
        self.package = self.root / "dependency"
        (self.package / "assets" / "fonts").mkdir(parents=True)
        (self.package / "Cargo.toml").write_text("fixture")
        (self.package / "LICENSE-MIT").write_text("upstream copyright and permission")
        (self.package / "assets" / "fonts" / "FiraSans-License.txt").write_text("font terms")
        self.output = self.root / "output"
        self.metadata = self.make_metadata()

    def make_metadata(self):
        packages = []
        for name in ("weftora-cli", "weftora-player", "dependency", "build", "dev-only"):
            root = self.repository if name.startswith("weftora") else self.package
            packages.append({"id": name, "name": name, "version": "1.0.0", "license": "MIT",
                             "manifest_path": str(root / "Cargo.toml"), "source": None})
        return {"packages": packages, "workspace_members": ["weftora-cli", "weftora-player"],
                "resolve": {"nodes": [
                    {"id": "weftora-cli", "deps": [
                        {"pkg": "dependency", "dep_kinds": [{"kind": None}]},
                        {"pkg": "build", "dep_kinds": [{"kind": "build"}]},
                        {"pkg": "dev-only", "dep_kinds": [{"kind": "dev"}]}]},
                    {"id": "weftora-player", "deps": [{"pkg": "dependency", "dep_kinds": [{"kind": None}]}]},
                    {"id": "dependency", "deps": [{"pkg": "build", "dep_kinds": [{"kind": None}]}]},
                    {"id": "build", "deps": []}, {"id": "dev-only", "deps": []}]}}

    def test_graph_includes_build_dependencies_once_and_excludes_dev_only(self):
        self.assertEqual([p["name"] for p in authoring.dependency_packages(self.metadata)],
                         ["build", "dependency", "weftora-cli", "weftora-player"])

    def test_notices_keep_nested_texts_and_hashes_with_workspace_licences(self):
        records = authoring.collect_notices(self.metadata, self.repository, self.output)
        dependency = next(record for record in records if record["name"] == "dependency")
        self.assertFalse(dependency["missingTextOrMetadata"])
        self.assertEqual(len(dependency["files"]), 2)
        for record in records:
            for file in record["files"]:
                self.assertEqual(authoring.digest(self.output / file["path"]), file["sha256"])
        self.assertEqual((self.output / "licenses/dependency-1.0.0/assets/fonts/FiraSans-License.txt").read_text(), "font terms")
        self.assertEqual(len(next(record for record in records if record["workspace"])["files"]), 2)

    def test_missing_text_or_metadata_is_explicitly_flagged(self):
        (self.package / "LICENSE-MIT").unlink()
        (self.package / "assets/fonts/FiraSans-License.txt").unlink()
        records = authoring.collect_notices(self.metadata, self.repository, self.output)
        self.assertTrue(next(record for record in records if record["name"] == "dependency")["missingTextOrMetadata"])
        self.metadata["packages"][0]["license"] = None
        records = authoring.collect_notices(self.metadata, self.repository, self.root / "second-output")
        self.assertTrue(next(record for record in records if record["name"] == "weftora-cli")["missingTextOrMetadata"])

    def test_declared_licence_cannot_escape_dependency_root(self):
        outside = self.root / "outside"
        outside.write_text("outside terms")
        with self.assertRaisesRegex(ValueError, "escapes"):
            authoring.licence_files(self.package, str(outside))

    def test_internal_resolving_traversal_is_canonicalized_before_copy(self):
        self.metadata["packages"][2]["license_file"] = "../dependency/LICENSE-MIT"
        records = authoring.collect_notices(self.metadata, self.repository, self.output)
        dependency = next(record for record in records if record["name"] == "dependency")
        self.assertEqual(len(dependency["files"]), 2)
        for file in dependency["files"]:
            self.assertNotIn("..", Path(file["path"]).parts)
            self.assertTrue((self.output / file["path"]).resolve().is_relative_to(self.output.resolve()))
        self.assertTrue((self.output / "licenses/dependency-1.0.0/LICENSE-MIT").is_file())

    def test_same_name_version_from_distinct_sources_cannot_overwrite_notices(self):
        first = dict(self.metadata["packages"][2])
        for name in (first["name"], first["name"].upper()):
            second = dict(first, id="different-source", name=name)
            with self.subTest(name=name), patch.object(authoring, "dependency_packages", return_value=[first, second]):
                with self.assertRaisesRegex(ValueError, "ambiguous package"):
                    authoring.collect_notices(self.metadata, self.repository, self.output)
            self.assertFalse(self.output.exists())

    def fallback_fixture(self, kind="licence-text"):
        root = self.repository / "third-party/notices"
        root.mkdir(parents=True)
        (root / "LICENSE-MIT").write_text("exact upstream copyright and terms")
        revision = "a" * 40
        (self.package / ".cargo_vcs_info.json").write_text(json.dumps({"git": {"sha1": revision}}))
        entry = {"name": "dependency", "version": "1.0.0", "source": None, "declaredLicense": "MIT",
                 "upstreamCommit": revision, "files": [{"path": "LICENSE-MIT", "sha256": authoring.digest(root / "LICENSE-MIT"),
                 "url": "https://example.test/" + revision + "/LICENSE-MIT", "kind": kind}]}
        (root / "index.json").write_text(json.dumps({"format": 1, "packages": [entry]}))
        return root, entry

    def test_pinned_fallback_copies_hash_and_provenance(self):
        self.fallback_fixture()
        (self.package / "LICENSE-MIT").unlink()
        records = authoring.collect_notices(self.metadata, self.repository, self.output)
        record = next(row for row in records if row["name"] == "dependency")
        self.assertFalse(record["missingTextOrMetadata"])
        copied = next(file for file in record["files"] if "upstream" in file)
        self.assertEqual(copied["upstream"]["commit"], "a" * 40)
        self.assertEqual(authoring.digest(self.output / copied["path"]), copied["sha256"])

    def test_fallback_rejects_changed_revision_text_metadata_and_escaped_path(self):
        root, entry = self.fallback_fixture()
        package = self.metadata["packages"][2]
        for change in ("revision", "text", "metadata", "path"):
            modified = json.loads(json.dumps(entry))
            if change == "revision":
                modified["upstreamCommit"] = "b" * 40
            elif change == "text":
                modified["files"][0]["sha256"] = "0" * 64
            elif change == "metadata":
                modified["declaredLicense"] = "Apache-2.0"
            else:
                modified["files"][0]["path"] = "../LICENSE-MIT"
            with self.subTest(change=change), self.assertRaises(ValueError):
                authoring.upstream_notice_files(package, self.package, root, {"packages": [modified]})

        self.assertEqual(authoring.upstream_notice_files(dict(package, version="2.0.0"), self.package, root, {"packages": [entry]}), [])
        (self.package / ".cargo_vcs_info.json").write_text(json.dumps({"git": {"sha1": "a" * 40, "dirty": True}}))
        with self.assertRaisesRegex(ValueError, "revision changed"):
            authoring.upstream_notice_files(package, self.package, root, {"packages": [entry]})

    def test_nested_notice_or_upstream_policy_does_not_resolve_missing_crate_licence(self):
        self.fallback_fixture(kind="licensing-policy")
        (self.package / "LICENSE-MIT").unlink()
        records = authoring.collect_notices(self.metadata, self.repository, self.output)
        self.assertTrue(next(row for row in records if row["name"] == "dependency")["missingTextOrMetadata"])

    def test_mismatched_tools_refuse_before_creating_output(self):
        cli, player = self.root / "weftora", self.root / "weftora-player"
        cli.write_bytes(b"CLI")
        player.write_bytes(b"player")
        with patch.object(authoring, "tool_identity", side_effect=[{"tool": "weftora", "buildId": "A"}, {"tool": "weftora-player", "buildId": "B"}]):
            with self.assertRaisesRegex(ValueError, "identities must match"):
                authoring.stage(cli, player, self.output)
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
