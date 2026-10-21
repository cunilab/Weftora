import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("drift", Path(__file__).resolve().parents[1] / "verify-tool-drift.py")
drift = importlib.util.module_from_spec(spec)
spec.loader.exec_module(drift)


class DriftTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_cache_copy_is_independent_and_skips_tests_and_incremental(self):
        cache, target = self.root / "cache", self.root / "target"
        for name in ("debug/deps/libdependency.rlib", "debug/deps/test.exe", "debug/deps/test-no-extension",
                     "debug/build/build-script.exe", "debug/.fingerprint/stamp", "debug/incremental/large"):
            path = cache / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"original")
        drift.seed_cache(cache, target, "debug", None)
        artifact = target / "debug/deps/libdependency.rlib"
        artifact.write_bytes(b"changed")
        self.assertEqual((cache / "debug/deps/libdependency.rlib").read_bytes(), b"original")
        self.assertFalse((target / "debug/incremental").exists())
        self.assertTrue((target / "debug/build/build-script.exe").is_file())
        if sys.platform == "darwin":
            self.assertFalse((target / "debug/deps/test-no-extension").exists())
        else:
            self.assertFalse((target / "debug/deps/test.exe").exists())

    def test_cache_overlap_and_empty_cache_refuse(self):
        cache = self.root / "cache"
        cache.mkdir()
        for target in (cache, cache / "nested", self.root):
            with self.assertRaisesRegex(SystemExit, "overlap"):
                drift.seed_cache(cache, target, "release", None)
        with self.assertRaisesRegex(SystemExit, "no artifacts"):
            drift.seed_cache(cache, self.root / "fresh", "release", None)

    def test_target_specific_cache_preserves_host_build_dependencies(self):
        cache, target = self.root / "cache", self.root / "target"
        for name in ("release/build/compiler.exe", "x86_64-pc-windows-msvc/release/deps/libcore.rlib"):
            path = cache / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"dependency")
        copied = drift.seed_cache(cache, target, "release", "x86_64-pc-windows-msvc")
        self.assertEqual(len(copied), 2)
        self.assertTrue((target / "release/build/compiler.exe").is_file())
        self.assertTrue((target / "x86_64-pc-windows-msvc/release/deps/libcore.rlib").is_file())

    @unittest.skipIf(os.name == "nt", "Windows symlinks require host privilege")
    def test_cache_symlink_refuses_before_copy(self):
        deps = self.root / "cache/debug/deps"
        deps.mkdir(parents=True)
        outside = self.root / "outside"
        outside.write_bytes(b"preserve")
        (deps / "libalias.rlib").symlink_to(outside)
        with self.assertRaisesRegex(SystemExit, "links/reparse"):
            drift.seed_cache(self.root / "cache", self.root / "target", "debug", None)
        self.assertEqual(outside.read_bytes(), b"preserve")
        self.assertFalse((self.root / "target/debug/deps").exists())

    def test_drift_requires_only_build_id_to_change(self):
        baseline = {"tool": "weftora", "buildId": "old", "profile": "release", "target": "target"}
        drift.changed_identity(baseline, dict(baseline, buildId="new"))
        for changed, reason in ((baseline, "did not change"), (dict(baseline, buildId="new", profile="debug"), "unexpected identity")):
            with self.assertRaisesRegex(SystemExit, reason):
                drift.changed_identity(baseline, changed)

    def test_build_requires_fresh_cli_in_isolated_target_and_checkout(self):
        workspace, target = self.root / "source", self.root / "target"
        workspace.mkdir()
        target.mkdir()
        executable = target / "weftora.exe"
        executable.write_bytes(b"fake compiler output")
        dep_info = executable.with_suffix(".d")
        dep_info.write_text(str(workspace / drift.CONTROLLED_SOURCE))
        message = {"reason": "compiler-artifact", "package_id": "path+file:///source#weftora-cli@0.0.1", "target": {"name": "weftora"}, "executable": str(executable), "fresh": False}
        library = {"reason": "compiler-artifact", "package_id": "path+file:///source#weftora-core@0.0.1", "target": {"name": "weftora_core"}, "fresh": False}

        def run(command, **kwargs):
            self.assertEqual(kwargs["cwd"], workspace)
            self.assertEqual(kwargs["env"]["CARGO_TARGET_DIR"], str(target))
            self.assertIn("--locked", command)
            self.assertEqual(command[-2:], ["--target", "x86_64-pc-windows-msvc"])
            kwargs["stdout"].write((json.dumps(library) + "\n" + json.dumps(message) + "\n").encode())
            return subprocess.CompletedProcess(command, 0)

        with mock.patch.object(drift.subprocess, "run", side_effect=run):
            actual, record = drift.build_cli(workspace, target, "release", "x86_64-pc-windows-msvc", self.root / "build.log")
            self.assertEqual(actual, executable.resolve())
            self.assertTrue(record["cliRebuilt"])
            library["fresh"] = True
            with self.assertRaisesRegex(SystemExit, "reused path-crate"):
                drift.build_cli(workspace, target, "release", "x86_64-pc-windows-msvc", self.root / "library.log", require_path_rebuild=True)
            library["fresh"] = False
            message["fresh"] = True
            with self.assertRaisesRegex(SystemExit, "actually be rebuilt"):
                drift.build_cli(workspace, target, "release", "x86_64-pc-windows-msvc", self.root / "fresh.log")
            message["fresh"] = False
            dep_info.write_text("another-checkout/source.rs")
            with self.assertRaisesRegex(SystemExit, "wrong checkout"):
                drift.build_cli(workspace, target, "release", "x86_64-pc-windows-msvc", self.root / "wrong.log")

    def test_cache_reset_cleans_workspace_and_vendored_packages_only_in_isolated_target(self):
        workspace, target = self.root / "workspace", self.root / "target"
        metadata = {"packages": [{"id": "path+workspace", "source": None}, {"id": "path+vendor", "source": None}, {"id": "registry+dependency", "source": "registry"}]}
        result = subprocess.CompletedProcess([], 0)
        with mock.patch.object(drift.subprocess, "check_output", return_value=json.dumps(metadata).encode()), mock.patch.object(drift.subprocess, "run", return_value=result) as run:
            record = drift.reset_path_cache(workspace, target, "release", "triple", self.root / "clean.log")
        self.assertEqual(record["pathPackages"], ["path+vendor", "path+workspace"])
        self.assertEqual(run.call_count, 2)
        self.assertNotIn("--target", run.call_args_list[0].args[0])
        command = run.call_args.args[0]
        self.assertNotIn("registry+dependency", command)
        self.assertEqual(command[command.index("--target") + 1], "triple")
        self.assertEqual(run.call_args.kwargs["env"]["CARGO_TARGET_DIR"], str(target))
        self.assertEqual(run.call_args.kwargs["cwd"], workspace)

    def test_output_inside_checkout_refuses_before_build(self):
        repository = Path(__file__).resolve().parents[2]
        output = repository / ("drift-refused-" + self.root.name)
        with mock.patch.object(drift.subprocess, "run") as run, self.assertRaisesRegex(SystemExit, "outside checkout"):
            drift.prove(Path(sys.executable), Path(sys.executable), output)
        run.assert_not_called()
        self.assertFalse(output.exists())

    def test_export_metadata_cannot_direct_cleanup_to_frozen_player(self):
        frozen = (self.root / "frozen").resolve()
        frozen.mkdir()
        cli, player = frozen / "weftora", frozen / "weftora-player"
        cli.write_bytes(b"frozen cli")
        player.write_bytes(b"frozen player")
        identity = {"tool": "weftora", "buildId": "baseline", "profile": "debug", "target": "host"}
        player_id = dict(identity, tool="weftora-player")

        def run(command, **kwargs):
            if command[1] == "--tool-identity":
                return subprocess.CompletedProcess(command, 0, stdout=json.dumps(identity if command[0] == str(cli) else player_id), stderr="")
            if command[-1] == "--validate":
                return subprocess.CompletedProcess(command, 0, stdout="", stderr="")
            self.assertEqual(command[1], "export")
            destination = Path(command[-1])
            destination.mkdir()
            meta = {"cli": {"identity": identity, "sha256": drift.digest(cli)},
                    "player": {"identity": player_id, "sha256": drift.digest(player), "file": "../../frozen/weftora-player"}}
            (destination / "weftora-export.json").write_text(json.dumps(meta))
            return subprocess.CompletedProcess(command, 0, stdout="", stderr="")

        with mock.patch.object(drift.source, "inventory", return_value={}), mock.patch.object(drift.source, "copy_workspace", return_value={}), mock.patch.object(drift.subprocess, "check_output", side_effect=["commit\n", b""]), mock.patch.object(drift.subprocess, "run", side_effect=run), self.assertRaisesRegex(SystemExit, "control player filename"):
            drift.prove(cli, player, self.root / "proof")
        self.assertEqual(player.read_bytes(), b"frozen player")


if __name__ == "__main__":
    unittest.main()
