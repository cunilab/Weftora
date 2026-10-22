#!/usr/bin/env python3
"""Build/test a fresh Windows candidate and record unsigned build evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

TARGET = "x86_64-pc-windows-msvc"
LINKER_ENV = "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def capture(command, cwd):
    return subprocess.run(command, cwd=cwd, check=True, capture_output=True,
                          text=True, encoding="utf-8", timeout=60).stdout.strip()


def fingerprint(path):
    require(path.is_file() and not path.is_symlink(), f"expected plain file: {path}")
    with path.open("rb") as source:
        hasher = hashlib.sha256()
        for block in iter(lambda: source.read(1024 * 1024), b""):
            hasher.update(block)
        digest = hasher.hexdigest()
    return {"bytes": path.stat().st_size, "sha256": digest}


def sources(root):
    require(not capture(["git", "status", "--porcelain", "--untracked-files=all", "--ignored", "--", ".", ":(exclude)target"], root),
            "candidate checkout must be clean, including untracked/ignored files outside target")
    names = capture(["git", "ls-files", "-z"], root).rstrip("\0").split("\0")
    require(names and names[0], "candidate has no tracked source files")
    return {name: fingerprint(root / name) for name in names}


def logged(command, root, output, name, env):
    print("Running " + " ".join(command), flush=True)
    with (output / name).open("xb") as log:
        subprocess.run(command, cwd=root, env=env, stdout=log,
                       stderr=subprocess.STDOUT, check=True)


def build(root, output, linker):
    root = root.resolve(strict=True)
    require(not os.path.lexists(root / "target"), "fresh build requires absent Cargo target directory")
    require(not os.path.lexists(output), "build evidence output must be fresh")
    output = output.resolve()
    require(not output.is_relative_to(root) and not root.is_relative_to(output), "evidence/source overlap")
    # shortcut: this is a controlled CI recipe, not a sandbox for arbitrary Cargo configuration.
    overrides = ("RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS",
                 "CL", "_CL_", "LINK", "_LINK_", "CC", "CXX", "CFLAGS", "CXXFLAGS", "HOST_CC", "TARGET_CC",
                 "HOST_CXX", "TARGET_CXX", "HOST_CFLAGS", "TARGET_CFLAGS", "HOST_CXXFLAGS", "TARGET_CXXFLAGS",
                 "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS", "CARGO_BUILD_RUSTDOCFLAGS",
                 "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR",
                 "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
                 "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_BUILD_DIR", "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS", LINKER_ENV)
    require(not any(os.environ.get(name) for name in overrides), "unexpected build/toolchain override")
    source = sources(root)
    required_inputs = ("Cargo.lock", "Cargo.toml", "rust-toolchain.toml", ".cargo/config.toml")
    require(all(name in source for name in required_inputs), "required build input is not tracked")
    revision = capture(["git", "rev-parse", "HEAD"], root)
    tree = capture(["git", "rev-parse", "HEAD^{tree}"], root)
    linker = linker.resolve(strict=True)
    require(linker.name.lower() == "link.exe", "expected explicit MSVC link.exe")
    compiler = linker.with_name("cl.exe")
    tools = {"cargo": Path(capture(["rustup", "which", "cargo"], root)),
             "rustc": Path(capture(["rustup", "which", "rustc"], root)),
             "rustdoc": Path(capture(["rustup", "which", "rustdoc"], root)),
             "linker": linker, "cCompiler": compiler}
    tool_hashes = {name: {"path": str(path), **fingerprint(path)} for name, path in tools.items()}
    versions = {name: capture([str(tools[name]), "--version", "--verbose"], root)
                for name in ("cargo", "rustc", "rustdoc")}
    env = dict(os.environ, RUSTC=str(tools["rustc"]), RUSTDOC=str(tools["rustdoc"]))
    env[LINKER_ENV] = str(linker)
    context_names = ("VCToolsVersion", "VCToolsInstallDir", "WindowsSDKVersion", "WindowsSdkDir",
                     "INCLUDE", "LIB", "LIBPATH", "ImageOS", "ImageVersion", "RUNNER_OS", "RUNNER_ARCH",
                     "UCRTVersion", "VSCMD_VER", "VSCMD_ARG_TGT_ARCH", "VSCMD_ARG_HOST_ARCH", "RUSTUP_TOOLCHAIN",
                     "GITHUB_EVENT_NAME", "GITHUB_REPOSITORY", "GITHUB_SHA", "GITHUB_REF", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT")
    environment = {name.upper(): value for name, value in env.items()}
    context = {name: environment[name.upper()] for name in context_names if name.upper() in environment}
    require(all(context.get(name, "").strip() for name in ("VCToolsVersion", "WindowsSDKVersion", "LIB", "INCLUDE")),
            "initialize the x64 Visual Studio developer shell first")
    # Hash local Cargo config without copying configuration values that may contain credentials.
    cargo_home = Path(env.get("CARGO_HOME", str(Path.home() / ".cargo")))
    config_paths = {folder / ".cargo" / name for folder in (root, *root.parents) for name in ("config", "config.toml")}
    config_paths.update(cargo_home / name for name in ("config", "config.toml"))
    configs = {str(path): fingerprint(path) for path in sorted(config_paths) if path.exists()}
    require(set(configs) == {str(root / ".cargo/config.toml")},
            "controlled CI recipe requires only the tracked workspace Cargo config")
    output.mkdir(parents=True, exist_ok=False)
    (output / "source-files.json").write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")
    for name, command in (("msvc-linker.txt", [str(linker), "/?"]), ("msvc-compiler.txt", [str(compiler), "/Bv"])):
        with (output / name).open("xb") as log:
            result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=30)
        require(result.returncode in ((0, 2) if name == "msvc-compiler.txt" else (0,)), "MSVC version capture failed")
        require((output / name).stat().st_size > 0, "missing MSVC version output")
    commands = [[str(tools["cargo"]), "test-all", "--release", "--target", TARGET, "--target-dir", str(root / "target"),
                 "--config", 'build.build-dir="target"', "--", "--test-threads=1"],
                [str(tools["cargo"]), "build", "--workspace", "--release", "--locked", "--target", TARGET, "--target-dir", str(root / "target"),
                 "--config", 'build.build-dir="target"', "--verbose"]]
    for command, name in zip(commands, ("release-tests.log", "release-build.log")):
        logged(command, root, output, name, env)
    binaries = {name: fingerprint(root / "target" / TARGET / "release" / name)
                for name in ("weftora.exe", "weftora-player.exe")}
    require(sources(root) == source and capture(["git", "rev-parse", "HEAD"], root) == revision,
            "source checkout changed during candidate build")
    require(all(fingerprint(tools[name]) == {k: row[k] for k in ("bytes", "sha256")}
                for name, row in tool_hashes.items()), "build tool changed")
    require({str(path): fingerprint(path) for path in sorted(config_paths) if path.exists()} == configs,
            "Cargo configuration changed")
    files = {path.name: fingerprint(path) for path in output.iterdir()}
    report = {"sourceCommit": revision, "sourceTree": tree, "target": TARGET, "profile": "release",
              "freshTarget": True, "tools": tool_hashes, "versions": versions, "environment": context,
              "explicitLinker": str(linker), "cargoConfiguration": configs, "commands": commands,
              "sourceInputs": {name: source[name] for name in required_inputs},
              "binaries": binaries, "evidenceFiles": files, "auditApproved": False,
              "scope": "Unsigned evidence from this fresh-target build/test recipe; tracked source bytes, selected executable tool files/settings and outputs; toolchain DLLs/sysroot libraries are not fully inventoried. Not a hermetic/reproducible-build or signed supply-chain attestation. Does not establish every native library input, exact minimum redistributable version, licence approval or native/clean-machine acceptance."}
    (output / "build-provenance.tmp").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "build-provenance.tmp").rename(output / "build-provenance.json")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--linker", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        require(os.name == "nt", "this recipe requires native Windows")
        build(Path.cwd(), args.out, args.linker)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"candidate build refused: {error}\n")


if __name__ == "__main__":
    main()
