#!/usr/bin/env python3
"""Rebuild an isolated CLI and prove it refuses the unchanged baseline player."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys

spec = importlib.util.spec_from_file_location("independence", Path(__file__).with_name("verify-engine-independence.py"))
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)
require, digest = source.require, source.digest
CONTROLLED_SOURCE = "apps/weftora-cli/src/tool_identity.rs"
APPEND = b"\n// P6-02 isolated build-fingerprint negative fixture.\n"


def seed_cache(cache, target, profile, triple):
    require(cache.resolve() != target.resolve() and not target.resolve().is_relative_to(cache.resolve())
            and not cache.resolve().is_relative_to(target.resolve()), "cache/build target overlap")
    profiles = [Path(profile)] + ([Path(triple) / profile] if triple else [])
    copied = []
    if sys.platform == "darwin":
        require(cache.stat().st_dev == target.parent.stat().st_dev, "macOS cache seeding requires the same filesystem")
    for relative in profiles:
        for name in ("deps", "build", ".fingerprint"):
            original, destination = cache / relative / name, target / relative / name
            if not original.is_dir():
                continue
            for directory, directories, files in os.walk(original, followlinks=False):
                for path in [Path(directory)] + [Path(directory) / item for item in directories + files]:
                    info = path.lstat()
                    require(not stat.S_ISLNK(info.st_mode) and not getattr(info, "st_file_attributes", 0) & 0x400,
                            "cache links/reparse points are not supported")
            require(not os.path.lexists(destination), "cache destination must be fresh")
            destination.parent.mkdir(parents=True, exist_ok=True)
            if sys.platform == "darwin":
                # APFS clones use independent copy-on-write extents, never hardlinks.
                subprocess.run(["cp", "-cRp", str(original), str(destination)], check=True)
                if name == "deps":
                    for path in destination.iterdir():
                        if path.is_file() and not path.suffix:
                            path.unlink()
            else:
                def ignore(directory, names):
                    return [item for item in names if name == "deps" and item.lower().endswith(".exe")]
                shutil.copytree(original, destination, ignore=ignore)
            copied.append((relative / name).as_posix())
    require(copied, "cache has no artifacts for selected profile/target")
    return copied


def changed_identity(baseline, changed):
    require(changed["buildId"] != baseline["buildId"], "controlled source change did not change build ID")
    require(changed == dict(baseline, buildId=changed["buildId"]), "unexpected identity fields changed")


def reset_path_cache(workspace, target, profile, triple, log):
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_TERM_COLOR="never")
    metadata_command = ["cargo", "metadata", "--locked", "--format-version", "1"]
    metadata = json.loads(subprocess.check_output(metadata_command, cwd=workspace, env=environment))
    packages = sorted(package["id"] for package in metadata["packages"] if package.get("source") is None)
    require(packages, "no path packages found for isolated cache reset")
    commands = []
    with log.open("wb") as output:
        # Reset host build scripts as well as an explicitly selected target layout.
        for selected in ([None, triple] if triple else [None]):
            command = ["cargo", "clean", "--locked", "--profile", profile]
            if selected:
                command += ["--target", selected]
            for package in packages:
                command += ["--package", package]
            result = subprocess.run(command, cwd=workspace, env=environment, stdout=output, stderr=subprocess.STDOUT, timeout=120)
            require(result.returncode == 0, f"isolated path cache reset failed: see {log.name}")
            commands.append(command)
    return {"metadataCommand": metadata_command, "commands": commands, "targetDirectory": str(target), "exitCode": 0, "pathPackages": packages}


def build_cli(workspace, target, profile, triple, log, require_path_rebuild=False):
    command = ["cargo", "build", "--locked", "-p", "weftora-cli", "--profile", profile, "--message-format=json"]
    if triple:
        command += ["--target", triple]
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_TERM_COLOR="never")
    with log.open("wb") as output:
        result = subprocess.run(command, cwd=workspace, env=environment, stdout=output, stderr=subprocess.STDOUT, timeout=3600)
    require(result.returncode == 0, f"CLI build failed: see {log.name}")
    artifacts, path_artifacts = [], []
    for line in log.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.startswith("{"):
            continue
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message.get("package_id", "").startswith("path+"):
            path_artifacts.append(message)
        if message.get("reason") == "compiler-artifact" and message.get("target", {}).get("name") == "weftora" and message.get("executable"):
            artifacts.append(message)
    require(len(artifacts) == 1 and artifacts[0]["fresh"] is False, "isolated CLI must actually be rebuilt")
    if require_path_rebuild:
        require(path_artifacts and all(message["fresh"] is False for message in path_artifacts), "unchanged control reused path-crate artifacts")
    executable = Path(artifacts[0]["executable"]).resolve(strict=True)
    require(executable.is_relative_to(target.resolve()), "CLI artifact escaped isolated build target")
    dep_info = executable.with_suffix(".d").read_text(encoding="utf-8").replace("\\ ", " ")
    require(source.message_has_path(dep_info, str(workspace / CONTROLLED_SOURCE)), "CLI dependency info points to wrong checkout")
    return executable, {"command": command, "cwd": str(workspace), "targetDirectory": str(target),
                        "exitCode": result.returncode, "cliRebuilt": True, "isolatedSourceInDepInfo": True,
                        "allPathArtifactsRebuilt": require_path_rebuild}


def prove(cli, player, output, triple=None, cache=None):
    repository = Path(__file__).resolve().parents[1]
    cli, player = cli.resolve(strict=True), player.resolve(strict=True)
    require(not os.path.lexists(output), "proof output must be fresh")
    output = output.resolve()
    require(not output.is_relative_to(repository), "proof output must be outside checkout")
    require(not triple or re.fullmatch(r"[A-Za-z0-9_-]+", triple), "expected a Rust target triple")
    environment = dict(os.environ, WEFTORA_PLAYER=str(player))

    def invoke(*command):
        return subprocess.run([str(part) for part in command], env=environment, capture_output=True, encoding="utf-8", timeout=120)

    def run(*command):
        result = invoke(*command)
        require(result.returncode == 0, result.stderr or result.stdout or f"failed: {command}")
        return result.stdout

    baseline = json.loads(run(cli, "--tool-identity"))
    player_id = json.loads(run(player, "--tool-identity"))
    require(baseline.get("tool") == "weftora" and player_id == dict(baseline, tool="weftora-player"), "baseline tool identities differ")
    require(baseline["profile"] in ("debug", "release"), "unsupported build profile")
    require(not triple or triple == baseline["target"], "build target differs from frozen tools")
    profile = "dev" if baseline["profile"] == "debug" else "release"
    cli_hash, player_hash = digest(cli), digest(player)
    records = source.inventory(repository)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repository, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=repository))
    output.mkdir(parents=True, exist_ok=False)
    workspace, target = output / "workspace", output / "target"
    copied = source.copy_workspace(repository, workspace, records)
    seeded = seed_cache(cache.resolve(strict=True), target, baseline["profile"], triple) if cache else []
    controls = []

    def positive(tool, label):
        for story in ("story-a", "life-sim"):
            destination = output / (label + "-" + story)
            run(tool, "export", repository / "examples" / story, "--out", destination)
            meta = json.loads((destination / "weftora-export.json").read_text(encoding="utf-8"))
            require(meta["cli"]["identity"] == baseline and meta["player"]["identity"] == player_id
                    and meta["cli"]["sha256"] == digest(tool) and meta["player"]["sha256"] == player_hash,
                    "matching control metadata mismatch")
            copied_player = destination / meta["player"]["file"]
            require(copied_player.name in ("weftora-player", "weftora-player.exe") and digest(copied_player) == player_hash, "control player hash mismatch")
            run(player, destination / "story", "--dialogue", destination / "compiled/dialogue.json", "--validate")
            copied_player.unlink()
            controls.append({"pair": label, "story": story, "exportAndValidationPassed": True, "playerSha256": player_hash})

    positive(cli, "original")
    reset = reset_path_cache(workspace, target, profile, triple, output / "path-cache-reset.log")
    print("Building unchanged isolated CLI control", flush=True)
    isolated_cli, unchanged_build = build_cli(workspace, target, profile, triple, output / "unchanged-build.log", require_path_rebuild=True)
    require(json.loads(run(isolated_cli, "--tool-identity")) == baseline, "unchanged isolated source does not reproduce baseline identity")
    isolated_hash = digest(isolated_cli)
    positive(isolated_cli, "isolated")
    controlled = workspace / CONTROLLED_SOURCE
    original_bytes = controlled.read_bytes()
    require(APPEND not in original_bytes, "controlled marker already exists")
    controlled.write_bytes(original_bytes + APPEND)
    changed_source_hash = digest(controlled)
    require({name for name, value in copied.items() if digest(workspace / name) != value} == {CONTROLLED_SOURCE}, "unexpected source-copy changes")
    print("Building changed isolated CLI", flush=True)
    changed_cli, changed_build = build_cli(workspace, target, profile, triple, output / "changed-build.log")
    identity = json.loads(run(changed_cli, "--tool-identity"))
    changed_identity(baseline, identity)
    changed_hash = digest(changed_cli)
    require(changed_hash != isolated_hash, "changed CLI bytes unexpectedly match control")
    refusals = []
    for story in ("story-a", "life-sim"):
        for existing in (False, True):
            destination = output / f"refused-{story}-{existing}"
            if existing:
                destination.mkdir()
            result = invoke(changed_cli, "export", repository / "examples" / story, "--out", destination)
            require(result.returncode == 1 and not result.stdout and "mismatched CLI/compiler/player" in result.stderr
                    and baseline["buildId"] in result.stderr and identity["buildId"] in result.stderr,
                    f"wrong changed-build refusal: {result.stderr}")
            require((destination.is_dir() and not any(destination.iterdir())) if existing else not os.path.lexists(destination), "refusal changed output")
            (output / f"refusal-{story}-{existing}.log").write_text(result.stderr, encoding="utf-8")
            refusals.append({"story": story, "existingEmptyOutput": existing, "exitCode": 1, "stdoutEmpty": True, "outputUnchanged": True})
    require(source.inventory(repository) == records, "original source inventory changed")
    require(digest(cli) == cli_hash and digest(player) == player_hash, "frozen input tools changed")
    require(digest(controlled) == changed_source_hash and all(digest(workspace / name) == value for name, value in copied.items() if name != CONTROLLED_SOURCE), "source copy changed during build")
    report = {"sourceCommit": commit, "workingTreeDirty": dirty,
              "sourceInventorySha256": hashlib.sha256(json.dumps(records, sort_keys=True).encode("utf-8")).hexdigest(),
              "baselineCliIdentity": baseline, "retainedPlayerIdentity": player_id, "changedCliIdentity": identity,
              "baselineCliSha256": cli_hash, "retainedPlayerSha256": player_hash, "isolatedUnchangedCliSha256": isolated_hash,
              "changedCliSha256": changed_hash, "controlledSource": CONTROLLED_SOURCE, "controlledAppend": APPEND.decode(),
              "baselineSourceSha256": copied[CONTROLLED_SOURCE], "changedSourceSha256": changed_source_hash,
              "pathCacheReset": reset, "builds": [unchanged_build, changed_build], "seededCacheDirectories": seeded,
              "matchingControls": controls, "refusals": refusals, "originalSourceAndToolsUnchanged": True,
              "scope": "Real unchanged/changed CLI builds in isolated source and target, paired with frozen player. Source copy omits A/B; controls/refusals use unchanged originals. No native/clean-machine/historical-release acceptance."}
    shutil.rmtree(target)
    shutil.rmtree(workspace)
    (output / "tool-drift-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--player", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--target")
    parser.add_argument("--cache-target", type=Path, help="Read-only Cargo artifacts to copy into the isolated target; no hardlinks")
    args = parser.parse_args()
    try:
        report = prove(args.cli, args.player, args.out, args.target, args.cache_target)
        print(f"Verified {len(report['refusals'])} actual changed-build refusals; frozen tools unchanged")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"tool-drift proof failed: {error}\n")


if __name__ == "__main__":
    main()
