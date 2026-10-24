import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("provenance", Path(__file__).resolve().parents[1] / "build-windows-candidate.py")
REAL_RUN = subprocess.run
provenance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provenance)


class BuildProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name).resolve()
        self.root = self.base / "source"
        self.root.mkdir()
        self.output = self.base / "evidence"
        for name in ("Cargo.lock", "Cargo.toml", "rust-toolchain.toml", ".cargo/config.toml"):
            path = self.root / name
            path.parent.mkdir(exist_ok=True)
            path.write_text("fixture " + name)
        self.tools = self.base / "tools"
        self.tools.mkdir()
        for name in ("cargo", "rustc", "rustdoc", "link.exe", "cl.exe"):
            (self.tools / name).write_bytes(name.encode())
        self.git("init")
        self.git("add", ".")
        self.git("-c", "commit.gpgsign=false", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "fixture")
        self.env = {key: value for key, value in os.environ.items() if key.upper() in
                    ("PATH", "SYSTEMROOT", "TEMP", "TMP", "HOME", "USERPROFILE")}
        self.env.update({"VCToolsVersion": "14.fixture", "WindowsSDKVersion": "fixture", "LIB": "fixture", "INCLUDE": "fixture", "CARGO_HOME": str(self.base / "cargo-home")})
        self.commands = []

    def git(self, *args):
        return REAL_RUN(["git", *args], cwd=self.root, capture_output=True, check=True)

    def capture(self, command, cwd):
        if command[0] == "git":
            return self.git(*command[1:]).stdout.decode().strip()
        if command[:2] == ["rustup", "which"]:
            return str(self.tools / command[2])
        return "fixture verbose tool version"

    def execute(self, command, **kwargs):
        self.assertIn(Path(command[0]).name, ("cl.exe", "link.exe"))
        compiler = Path(command[0]).name == "cl.exe"
        banner = b"Microsoft (R) C/C++ Optimizing Compiler Version 19.51.fixture" if compiler else b"Microsoft (R) Incremental Linker Version 14.51.fixture"
        kwargs["stdout"].write(banner + b"\n")
        return subprocess.CompletedProcess(command, 2 if compiler else 1100)

    def logged(self, command, root, output, name, env):
        self.commands.append(command)
        self.assertEqual(env[provenance.LINKER_ENV], str(self.tools / "link.exe"))
        (output / name).write_text("fixture build/test log")
        if "build" in command:
            folder = root / "target" / provenance.TARGET / "release"
            folder.mkdir(parents=True)
            for tool in ("weftora.exe", "weftora-player.exe"):
                (folder / tool).write_bytes(tool.encode())

    def run_build(self, logged=None):
        with patch.dict(os.environ, self.env, clear=True), \
                patch.object(provenance, "capture", side_effect=self.capture), \
                patch.object(provenance, "logged", side_effect=logged or self.logged), \
                patch.object(provenance.subprocess, "run", side_effect=self.execute):
            return provenance.build(self.root, self.output, self.tools / "link.exe")

    def test_success_records_source_tools_binaries_and_evidence_hashes(self):
        report = self.run_build()
        self.assertFalse(report["auditApproved"])
        self.assertEqual(report["versions"]["msvc-linker.txt"]["returnCode"], 1100)
        self.assertEqual(report["versions"]["msvc-compiler.txt"]["returnCode"], 2)
        self.assertTrue(report["freshTarget"])
        self.assertEqual(len(report["binaries"]), 2)
        self.assertEqual(len(self.commands), 2)
        self.assertIn("--locked", self.commands[1])
        for command in self.commands:
            self.assertEqual(command[command.index("--target-dir") + 1], str(self.root / "target"))
            self.assertEqual(command[command.index("--config") + 1], 'build.build-dir="target"')
        sources = json.loads((self.output / "source-files.json").read_text())
        self.assertEqual(sources["Cargo.lock"], provenance.fingerprint(self.root / "Cargo.lock"))
        self.assertEqual(report["evidenceFiles"]["source-files.json"], provenance.fingerprint(self.output / "source-files.json"))
        self.assertEqual(json.loads((self.output / "build-provenance.json").read_text()), report)

    def test_missing_msvc_banner_never_produces_success_report(self):
        def command(args, **kwargs):
            kwargs["stdout"].write(b"tool failed without version evidence")
            return subprocess.CompletedProcess(args, 1)
        self.execute = command
        with self.assertRaisesRegex(ValueError, "version banner"):
            self.run_build()
        self.assertFalse((self.output / "build-provenance.json").exists())

    def test_windows_uppercase_environment_preserves_developer_shell_context(self):
        self.env = {name.upper(): value for name, value in self.env.items()}
        report = self.run_build()
        self.assertEqual(report["environment"]["VCToolsVersion"], "14.fixture")
        self.assertEqual(report["environment"]["WindowsSDKVersion"], "fixture")

    def test_dirty_checkout_target_existing_output_and_overrides_refuse(self):
        for refusal in ("dirty", "ignored", "target", "output", "override", "external-config"):
            with self.subTest(refusal=refusal):
                if refusal == "dirty":
                    (self.root / "extra.txt").write_text("untracked")
                elif refusal == "ignored":
                    (self.root / ".git/info/exclude").write_text("ignored.txt\n")
                    (self.root / "ignored.txt").write_text("ignored input")
                elif refusal == "external-config":
                    path = self.base / "cargo-home/config.toml"
                    path.parent.mkdir()
                    path.write_text("[build]\ntarget-dir='elsewhere'")
                elif refusal == "target":
                    (self.root / "target").mkdir()
                elif refusal == "output":
                    self.output.mkdir()
                else:
                    self.env["RUSTC_WRAPPER"] = "unexpected"
                with self.assertRaises(ValueError):
                    self.run_build()
                self.assertFalse((self.output / "build-provenance.json").exists())
                if refusal == "dirty":
                    (self.root / "extra.txt").unlink()
                elif refusal == "ignored":
                    (self.root / "ignored.txt").unlink()
                elif refusal == "external-config":
                    path.unlink()
                    path.parent.rmdir()
                elif refusal == "target":
                    (self.root / "target").rmdir()
                elif refusal == "output":
                    self.output.rmdir()
                else:
                    del self.env["RUSTC_WRAPPER"]

    def test_msvc_and_native_compiler_flags_refuse_before_output(self):
        for name in ("CL", "LINK", "_CL_", "_LINK_", "CC", "CXX", "CFLAGS", "CXXFLAGS", "RUSTDOCFLAGS"):
            with self.subTest(name=name):
                self.env[name] = "unexpected"
                with self.assertRaisesRegex(ValueError, "override"):
                    self.run_build()
                self.assertFalse(self.output.exists())
                del self.env[name]

    def test_source_tool_mutation_or_failed_build_has_no_success_marker(self):
        for failure in ("source", "tool", "test-command", "build-command", "new-config"):
            with self.subTest(failure=failure):
                output = self.base / ("out-" + failure)
                self.output = output
                def changed(command, root, out, name, env):
                    if failure == "test-command" or (failure == "build-command" and "build" in command):
                        raise subprocess.CalledProcessError(1, command)
                    self.logged(command, root, out, name, env)
                    if "build" in command:
                        if failure == "new-config":
                            path = self.base / "cargo-home/config.toml"
                            path.parent.mkdir()
                            path.write_text("[build]\ntarget-dir='elsewhere'")
                        else:
                            path = root / "Cargo.lock" if failure == "source" else self.tools / "link.exe"
                            path.write_bytes(b"changed")
                expected = {"source": "checkout must be clean", "tool": "build tool changed",
                            "new-config": "Cargo configuration changed"}.get(failure, "non-zero exit status")
                with self.assertRaisesRegex((ValueError, subprocess.CalledProcessError), expected):
                    self.run_build(changed)
                self.assertFalse((output / "build-provenance.json").exists())
                self.git("checkout", "--", "Cargo.lock")
                (self.tools / "link.exe").write_bytes(b"link.exe")
                target = self.root / "target"
                if target.exists():
                    import shutil
                    shutil.rmtree(target)
