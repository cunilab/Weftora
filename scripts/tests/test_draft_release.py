import copy
from email.message import Message
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError
import zipfile

spec = importlib.util.spec_from_file_location("draft_release", Path(__file__).resolve().parents[1] / "prepare-github-release.py")
draft = importlib.util.module_from_spec(spec)
spec.loader.exec_module(draft)
REAL_RUN = subprocess.run


class FakeGitHub:
    def __init__(self, case):
        self.case = case
        self.run_reads = 0
        self.release = None
        self.before_second_read = None

    def json(self, path):
        case = self.case
        if path == "actions/runs/101":
            self.run_reads += 1
            if self.run_reads == 2 and self.before_second_read:
                self.before_second_read()
            return copy.deepcopy(case.run)
        if path == "git/ref/tags/" + draft.quote(case.tag, safe=""):
            return {"object": copy.deepcopy(case.tag_ref)}
        if path == "git/tags/" + case.tag_object:
            return {"object": {"type": "commit", "sha": case.commit}}
        if path == "git/ref/heads/main":
            return {"object": {"type": "commit", "sha": case.main}}
        if path == "releases/99":
            return copy.deepcopy(self.release)
        raise AssertionError("unexpected API request: " + path)

    def pages(self, path, key=None):
        case = self.case
        if path.startswith("actions/runs/101/attempts/"):
            return copy.deepcopy(case.jobs)
        if path == "actions/runs/101/artifacts":
            return copy.deepcopy(case.artifacts)
        if path == "releases":
            return copy.deepcopy(case.existing_releases + ([self.release] if self.release else []))
        raise AssertionError("unexpected listing: " + path)

    def download(self, artifact, destination, limit):
        source = self.case.containers[artifact["id"]]
        assert source.stat().st_size <= limit
        shutil.copyfile(source, destination)


@unittest.skipIf(draft.tomllib is None, "draft-release helper requires Python 3.11+")
class DraftReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name).resolve()
        self.set_candidate("0.1.0")

    def git(self, *args):
        return REAL_RUN(["git", *args], cwd=self.root, capture_output=True, check=True).stdout.decode().strip()

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(text.encode())

    def set_candidate(self, version):
        self.version, self.tag = version, "v" + version
        self.root = self.base / ("source-" + version)
        self.root.mkdir()
        self.git("init", "-b", "main")
        for key, value in (("user.name", "Fixture"), ("user.email", "fixture@example.invalid"),
                           ("commit.gpgsign", "false"), ("tag.gpgsign", "false"), ("core.autocrlf", "false"),
                           ("core.hooksPath", str(self.root / ".git/no-hooks"))):
            self.git("config", key, value)
        self.write("Cargo.toml", '[workspace]\nmembers=["apps/cli","apps/player"]\n[workspace.package]\nversion="' + version + '"\n')
        self.write("Cargo.lock", 'version=4\n' + ''.join('[[package]]\nname="' + name + '"\nversion="' + ver + '"\n' for name, ver in (("weftora-cli", version), ("weftora-player", version), ("vendored-path-package", "0.9.0"))))
        self.write("rust-toolchain.toml", '[toolchain]\nchannel="1.98.0"\n')
        self.write(".cargo/config.toml", '[alias]\ntest-all="test --workspace --locked"\n')
        for member, name in (("cli", "weftora-cli"), ("player", "weftora-player")):
            self.write("apps/" + member + "/Cargo.toml", '[package]\nname="' + name + '"\nversion.workspace=true\n')
        self.git("add", ".");self.git("commit", "-m", "fixture")
        self.commit = self.git("rev-parse", "HEAD")
        self.main = self.commit
        self.git("tag", "-a", self.tag, "-m", "fixture release")
        self.tag_object = self.git("rev-parse", "refs/tags/" + self.tag)
        self.tag_ref = {"type": "tag", "sha": self.tag_object}
        self.output = self.base / ("output-" + version)
        self.notes = self.base / "notes.md";self.notes.write_text("Reviewed notes and supported contracts.\n")
        self.run = {"event": "push", "head_branch": "main", "path": ".github/workflows/ci.yml", "repository": {"full_name": draft.REPO}, "head_repository": {"full_name": draft.REPO}, "status": "completed", "conclusion": "success", "run_attempt": 1, "head_sha": self.commit}
        self.jobs = [{"name": name, "status": "completed", "conclusion": "success"} for name in draft.JOBS]
        self.jobs.append({"name": "macOS headless (non-blocking)", "status": "completed", "conclusion": "failure"})
        self.existing_releases = []
        self.artifacts, self.containers, self.commands = [], {}, []
        self.build_dir = self.base / ("build-" + version);self.build_dir.mkdir()
        source = {name: draft.fingerprint(self.root / name) for name in self.git("ls-files").splitlines()}
        (self.build_dir / "source-files.json").write_text(json.dumps(source))
        for name in draft.BUILD_FILES - {"source-files.json", "build-provenance.json"}:
            (self.build_dir / name).write_text("fixture build evidence")
        self.binary_bytes = {"weftora.exe": b"fixture-cli", "weftora-player.exe": b"fixture-player"}
        binaries = {name: {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()} for name, data in self.binary_bytes.items()}
        self.build = {"sourceCommit": self.commit, "sourceTree": self.git("rev-parse", "HEAD^{tree}"), "environment": {"GITHUB_RUN_ID": "101", "GITHUB_RUN_ATTEMPT": "1"}, "target": "x86_64-pc-windows-msvc", "profile": "release", "freshTarget": True, "auditApproved": False, "evidenceFiles": {name: draft.fingerprint(self.build_dir / name) for name in draft.BUILD_FILES - {"build-provenance.json"}}, "sourceInputs": {name: source[name] for name in ("Cargo.lock", "Cargo.toml", "rust-toolchain.toml", ".cargo/config.toml")}, "binaries": binaries}
        (self.build_dir / "build-provenance.json").write_text(json.dumps(self.build))
        identity = {"tool": "weftora", "engineRelease": version, "target": self.build["target"], "profile": "release", "buildId": "fixture"}
        self.notice = {"target": self.build["target"], "profile": "release", "auditApproved": False, "cliIdentity": identity, "playerIdentity": dict(identity, tool="weftora-player"), "cargoLockSha256": source["Cargo.lock"]["sha256"], "cliSha256": binaries["weftora.exe"]["sha256"], "playerSha256": binaries["weftora-player.exe"]["sha256"], "packages": [{"name": "fixture-dependency", "version": "1.0.0", "missingTextOrMetadata": False}]}
        self.package_folders = []
        for name in sorted(draft.ZIP_NAMES):
            folder = self.base / (version + "-packages") / name[:-4];folder.mkdir(parents=True)
            toolkit = name == "weftora-authoring-tools.zip"
            for role, data in self.binary_bytes.items():
                if toolkit or role == "weftora-player.exe":(folder / role).write_bytes(data)
            notice = folder / ("dependency-notices.json" if toolkit else "notices/dependency-notices.json")
            notice.parent.mkdir(exist_ok=True);notice.write_text(json.dumps(self.notice))
            if not toolkit:
                (folder / "weftora-export.json").write_text(json.dumps({"cli": {"identity": identity, "sha256": self.notice["cliSha256"]}, "player": {"identity": self.notice["playerIdentity"], "sha256": self.notice["playerSha256"]}}))
            transport = folder.parent / (name[:-4] + "-transport")
            draft.assets.transport.package(folder, transport)
            self.package_folders.append(transport)
        self.downloads = self.base / ("downloads-" + version)
        draft.assets.stage(self.package_folders, self.downloads)
        self.wrap("build-provenance", self.build_dir, 1)
        self.wrap("download-candidate", self.downloads, 2)
        self.client = FakeGitHub(self)

    def wrap(self, kind, folder, artifact_id):
        container = self.base / (self.version + "-" + kind + ".zip")
        with zipfile.ZipFile(container, "w", compression=zipfile.ZIP_DEFLATED) as z:
            for path in folder.iterdir():z.write(path, path.name)
        self.containers[artifact_id] = container
        row = {"id": artifact_id, "name": f"weftora-windows-{kind}-{self.commit}-attempt-1", "expired": False, "workflow_run": {"id": 101, "head_sha": self.commit}, "digest": "sha256:" + draft.fingerprint(container)["sha256"], "size_in_bytes": container.stat().st_size}
        self.artifacts = [a for a in self.artifacts if a["id"] != artifact_id] + [row]

    def command(self, command, **kwargs):
        if command[0] == "git":return REAL_RUN(command, **kwargs)
        self.assertEqual(command[:3], ["gh", "release", "create"])
        self.assertIn("--draft", command);self.assertIn("--verify-tag", command);self.assertIn("--latest=false", command)
        self.commands.append(command)
        self.client.release = {"id": 99, "tag_name": self.tag, "draft": True, "prerelease": "--prerelease" in command, "html_url": "https://github.com/fixture/draft", "assets": [{"name": path.name, "state": "uploaded", "size": path.stat().st_size, "digest": "sha256:" + draft.fingerprint(path)["sha256"]} for path in (self.output / "downloads").iterdir() if path.name in draft.ZIP_NAMES | {"SHA256SUMS"}]}
        return subprocess.CompletedProcess(command, 0)

    def prepare(self, create=False, reviewed=False):
        with patch.object(draft.subprocess, "run", side_effect=self.command):
            return draft.prepare(self.root, self.client, self.tag, 101, self.notes, self.output, create, reviewed)

    def test_full_preflight_and_create_only_draft_without_executing_payloads(self):
        plan = self.prepare()
        self.assertFalse(plan["draftCreated"]);self.assertFalse(plan["auditApproved"]);self.assertEqual(self.commands, [])
        self.assertTrue((self.output / "draft-release-plan.json").exists())
        self.output = self.base / "created"
        result = self.prepare(create=True, reviewed=True)
        self.assertTrue(result["draftCreated"]);self.assertEqual(len(self.commands), 1)
        self.assertEqual(len(result["assets"]), 4)
        self.assertTrue((self.output / "draft-created.json").exists())
        self.assertNotIn("--prerelease", self.commands[0])

    def test_prerelease_flag_is_derived_from_tag(self):
        self.set_candidate("0.1.0-rc.1+build")
        self.prepare(create=True, reviewed=True)
        self.assertIn("--prerelease", self.commands[0])

    def test_bad_ci_sources_attempts_and_jobs_cannot_create(self):
        for field, value, message in (("event", "pull_request", "push run from main"), ("head_branch", "feature", "push run from main"), ("conclusion", "failure", "complete and successful"), ("run_attempt", 2, "attempt-qualified artifact"), ("head_sha", "0" * 40, "CI and tag commit differ")):
            with self.subTest(field=field):
                before = self.run[field];self.run[field] = value
                with self.assertRaisesRegex(ValueError, message):self.prepare(True, True)
                self.run[field] = before
                self.assertFalse(self.output.exists());self.assertEqual(self.commands, [])
        self.jobs[0]["conclusion"] = "skipped"
        with self.assertRaisesRegex(ValueError, "required CI job"):self.prepare(True, True)
        self.assertEqual(self.commands, [])

    def test_tag_and_main_checks_cannot_be_bypassed(self):
        self.tag_ref["type"] = "commit"
        with self.assertRaisesRegex(ValueError, "annotated"):self.prepare()
        self.tag_ref["type"] = "tag"
        self.main = "0" * 40
        with self.assertRaises(subprocess.CalledProcessError):self.prepare()
        self.main = self.commit
        original = self.client.json
        def nested(path):
            if path == "git/tags/" + self.tag_object:return {"object": {"type": "tag", "sha": self.tag_object}}
            return original(path)
        with patch.object(self.client, "json", side_effect=nested), self.assertRaisesRegex(ValueError, "directly to a commit"):
            self.prepare()
        self.assertEqual(self.commands, [])

    def test_workspace_and_lockfile_version_mismatches_refuse(self):
        with self.assertRaisesRegex(ValueError, "tag and workspace version differ"):
            draft.source_versions(self.root, self.commit, "0.2.0")
        self.write("Cargo.lock", (self.root / "Cargo.lock").read_text().replace('version="0.1.0"', 'version="0.2.0"'))
        self.git("add", "Cargo.lock");self.git("commit", "-m", "mismatched lock fixture")
        with self.assertRaisesRegex(ValueError, "workspace/lockfile versions differ"):
            draft.source_versions(self.root, self.git("rev-parse", "HEAD"), "0.1.0")
        self.assertEqual(self.commands, [])

    def test_existing_draft_expired_or_ambiguous_artifacts_and_missing_attestation_refuse(self):
        with self.assertRaisesRegex(ValueError, "attestation"):self.prepare(True, False)
        self.existing_releases = [{"id": 5, "tag_name": self.tag, "draft": True}]
        with self.assertRaisesRegex(ValueError, "already exists"):self.prepare(True, True)
        self.existing_releases = []
        self.artifacts[0]["expired"] = True
        with self.assertRaisesRegex(ValueError, "expired"):self.prepare()
        self.artifacts[0]["expired"] = False
        self.artifacts.append(copy.deepcopy(self.artifacts[0]))
        with self.assertRaisesRegex(ValueError, "ambiguous"):self.prepare()
        self.assertEqual(self.commands, [])

    def test_source_manifest_or_build_revision_mismatch_refuses(self):
        for change in ("source", "commit"):
            with self.subTest(change=change):
                self.output = self.base / ("mismatch-" + change)
                original = (self.build_dir / "build-provenance.json").read_bytes()
                build = json.loads(original)
                if change == "source":
                    manifest = json.loads((self.build_dir / "source-files.json").read_text())
                    manifest["Cargo.toml"]["sha256"] = "0" * 64
                    (self.build_dir / "source-files.json").write_text(json.dumps(manifest))
                    build["evidenceFiles"]["source-files.json"] = draft.fingerprint(self.build_dir / "source-files.json")
                else:build["sourceCommit"] = "0" * 40
                (self.build_dir / "build-provenance.json").write_text(json.dumps(build));self.wrap("build-provenance", self.build_dir, 1)
                with self.assertRaisesRegex(ValueError, "build source"):self.prepare(True, True)
                self.assertFalse((self.output / "draft-release-plan.json").exists());self.assertEqual(self.commands, [])
                (self.build_dir / "build-provenance.json").write_bytes(original)

    def test_known_notice_gap_blocks_draft_creation(self):
        # Update one transport honestly so only the unresolved licence flag causes refusal.
        folder = self.base / (self.version + "-packages") / "weftora-authoring-tools"
        notice = copy.deepcopy(self.notice);notice["packages"][0]["missingTextOrMetadata"] = True
        (folder / "dependency-notices.json").write_text(json.dumps(notice))
        replacement = self.base / "new-toolkit-transport";draft.assets.transport.package(folder, replacement)
        packages = [replacement if p.name == "weftora-authoring-tools-transport" else p for p in self.package_folders]
        shutil.rmtree(self.downloads);draft.assets.stage(packages, self.downloads);self.wrap("download-candidate", self.downloads, 2)
        with self.assertRaisesRegex(ValueError, "unresolved notice text/metadata"):self.prepare(True, True)
        self.assertEqual(self.commands, []);self.assertFalse((self.output / "draft-created.json").exists())

    def test_changed_inputs_or_remote_attempt_before_create_never_mutate(self):
        for mode in ("notes", "attempt"):
            with self.subTest(mode=mode):
                self.output = self.base / ("changed-" + mode)
                self.client.run_reads = 0
                def changed():
                    if mode == "notes":(self.output / "release-notes.md").write_text("changed")
                    else:self.run["run_attempt"] = 2
                self.client.before_second_read = changed
                with self.assertRaisesRegex(ValueError, "changed after preflight|attempt-qualified artifact"):
                    self.prepare(True, True)
                self.assertEqual(self.commands, [])
                self.run["run_attempt"] = 1

    def test_postcreate_mismatch_leaves_draft_for_inspection_without_delete(self):
        original = self.client.json
        def wrong(path):
            result = original(path)
            if path == "releases/99":result["assets"][0]["digest"] = "sha256:" + "0" * 64
            return result
        with patch.object(self.client, "json", side_effect=wrong), self.assertRaisesRegex(ValueError, "draft asset readback differs"):
            self.prepare(True, True)
        self.assertEqual(len(self.commands), 1)
        self.assertFalse((self.output / "draft-created.json").exists())

    def test_raw_blob_comparison_ignores_archive_export_substitution(self):
        self.write(".gitattributes", "Cargo.toml export-subst\n")
        self.write("Cargo.toml", (self.root / "Cargo.toml").read_text() + "# $Format:%H$\n")
        self.git("add", ".");self.git("commit", "-m", "export substitution fixture")
        commit = self.git("rev-parse", "HEAD")
        manifest = {name: draft.fingerprint(self.root / name) for name in self.git("ls-files").splitlines()}
        draft.check_sources(self.root, commit, manifest)
        manifest["Cargo.toml"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "source bytes differ from Git"):
            draft.check_sources(self.root, commit, manifest)


class DraftBoundaryTests(unittest.TestCase):
    def test_release_listing_checks_later_pages(self):
        client = draft.GitHub.__new__(draft.GitHub)
        first = [{"tag_name": "v0.0." + str(i)} for i in range(100)]
        last = {"tag_name": "v0.1.0", "draft": True}
        with patch.object(client, "json", side_effect=[first, [last]]) as request:
            self.assertEqual(client.pages("releases"), first + [last])
        self.assertEqual(request.call_args_list[-1].args, ("releases?per_page=100&page=2",))

    def test_semver_rejects_ambiguous_versions_and_classifies_prereleases(self):
        for tag in ("v0.1.0", "v0.1.0-rc.1+build", "v1.2.3+build-5"):
            version, pre = draft.release_version(tag)
            self.assertEqual(version, tag[1:]);self.assertEqual(pre, tag == "v0.1.0-rc.1+build")
        for tag in ("1.2.3", "v01.2.3", "v1.2.3-01", "v1.2.3-", "v1.2.3+", "v1.2.3/escape", "v1.2.3\n"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):draft.release_version(tag)

    def test_artifact_redirect_strips_auth_and_checks_size_digest_host(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {"GH_TOKEN": "fixture-token"}):
            root = Path(directory);client = draft.GitHub();payload = b"artifact bytes"
            artifact = {"id": 1, "size_in_bytes": len(payload), "digest": "sha256:" + hashlib.sha256(payload).hexdigest()}
            requests = []
            class Response(io.BytesIO):
                pass
            def request(req, timeout):
                requests.append(req)
                if len(requests) == 1:
                    self.assertEqual(req.get_header("Authorization"), "Bearer fixture-token")
                    headers = Message();headers["Location"] = "https://unit.blob.core.windows.net/artifact?sig=fixture"
                    raise HTTPError(req.full_url, 302, "redirect", headers, io.BytesIO())
                self.assertIsNone(req.get_header("Authorization"))
                return Response(payload)
            with patch.object(client.opener, "open", side_effect=request):
                client.download(artifact, root / "valid.zip", 1024)
            self.assertEqual((root / "valid.zip").read_bytes(), payload)
            for mode in ("oversize", "checksum", "missing-digest"):
                requests.clear();bad = dict(artifact)
                if mode == "oversize":bad["size_in_bytes"] -= 1
                else:bad["digest"] = None if mode == "missing-digest" else "sha256:" + "0" * 64
                with self.subTest(mode=mode), patch.object(client.opener, "open", side_effect=request), self.assertRaises(ValueError):
                    client.download(bad, root / (mode + ".zip"), 1024)
            requests.clear()
            def bad_host(req, timeout):
                headers = Message();headers["Location"] = "https://untrusted.invalid/artifact"
                raise HTTPError(req.full_url, 302, "redirect", headers, io.BytesIO())
            with patch.object(client.opener, "open", side_effect=bad_host), self.assertRaisesRegex(ValueError, "unrecognized"):
                client.download(artifact, root / "bad-host.zip", 1024)

    def test_outer_zip_path_links_duplicates_and_expansion_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for mode in ("path", "link", "duplicate", "expansion"):
                archive = root / (mode + ".zip")
                with zipfile.ZipFile(archive, "w") as z:
                    entry = zipfile.ZipInfo("../file" if mode == "path" else "file")
                    if mode == "link":entry.external_attr = (0o120777 << 16)
                    z.writestr(entry, b"data")
                    if mode == "duplicate":
                        import warnings
                        with warnings.catch_warnings():
                            warnings.simplefilter("ignore", UserWarning);z.writestr("file", b"duplicate")
                with self.subTest(mode=mode), self.assertRaises(ValueError):
                    draft.unpack(archive, root / mode, {"file"}, 1 if mode == "expansion" else 1024)
                self.assertFalse((root / mode).exists())
