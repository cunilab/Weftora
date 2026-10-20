#!/usr/bin/env python3
"""Create, check, play headlessly and export a starter with existing tools."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def inventory(root):
    result = {}
    for path in sorted(root.rglob("*")):
        require(not path.is_symlink() and (path.is_file() or path.is_dir()), f"unexpected entry: {path}")
        result[path.relative_to(root).as_posix()] = None if path.is_dir() else digest(path)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--player", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    cli, player, output = args.cli.resolve(strict=True), args.player.resolve(strict=True), args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    environment = dict(os.environ, WEFTORA_PLAYER=str(player))
    cli_hash, player_hash = digest(cli), digest(player)

    def invoke(*command):
        return subprocess.run([str(part) for part in command], env=environment,
                              capture_output=True, encoding="utf-8", timeout=120)

    def run(*command):
        result = invoke(*command)
        require(result.returncode == 0, result.stderr or result.stdout or f"failed: {command}")
        return result.stdout

    cli_identity = json.loads(run(cli, "--tool-identity"))
    player_identity = json.loads(run(player, "--tool-identity"))
    expected = dict(cli_identity, tool="weftora-player")
    require(player_identity == expected, "CLI/player identities differ")
    story = output / "New Story ü"
    (output / "new.log").write_text(run(cli, "new", story), encoding="utf-8")
    before = inventory(story)
    require(set(before) == {"manifest.jsonc", "main.yarn", "README.md"}, "unexpected generated project files")
    (output / "check.log").write_text(run(cli, "check", story), encoding="utf-8")
    branches = []
    for choice, ready in [(0, True), (1, False)]:
        trace = run(cli, "run", story, "--headless", "--choices", choice)
        (output / f"branch-{choice}.jsonl").write_text(trace, encoding="utf-8")
        events = [json.loads(line) for line in trace.splitlines()]
        require(events[-1]["event"] == "end" and events[-1]["state"]["ready"] is ready,
                f"branch {choice}: wrong final state")
        branches.append({"choice":choice, "ready":ready, "completed":True})
    refused = invoke(cli, "new", story)
    require(refused.returncode == 1 and not refused.stdout and "already exists" in refused.stderr,
            "existing project must refuse without stdout")
    (output / "existing-project.log").write_text(refused.stderr, encoding="utf-8")
    game = output / "game"
    (output / "export.log").write_text(run(cli, "export", story, "--out", game), encoding="utf-8")
    metadata = json.loads((game / "weftora-export.json").read_text(encoding="utf-8"))
    require(metadata["cli"]["identity"] == cli_identity and metadata["player"]["identity"] == player_identity,
            "export identity metadata mismatch")
    require(metadata["cli"]["sha256"] == cli_hash and metadata["player"]["sha256"] == player_hash,
            "export tool hash metadata mismatch")
    executable = game / ("weftora-player.exe" if os.name == "nt" else "weftora-player")
    require(digest(executable) == player_hash, "export changed the player")
    (output / "validate.log").write_text(run(executable, game / "story", "--validate"), encoding="utf-8")
    require(inventory(game / "story") == before == inventory(story), "workflow changed generated source")
    require(digest(cli) == cli_hash and digest(player) == player_hash, "proof input tools changed")
    shutil.copy2(cli, output / cli.name)
    require(digest(output / cli.name) == cli_hash, "retained CLI changed")
    report = {"check":"P6-01 automated starter workflow", "platform":platform.platform(),
              "cliSha256":cli_hash, "playerSha256":player_hash, "cliIdentity":cli_identity, "playerIdentity":player_identity, "generatedFiles":before,
              "branches":branches, "existingProjectRefusedWithoutWrites":True,
              "sourceUnchanged":True, "exportedStoryMatchesSource":True, "standaloneValidationPassed":True,
              "scope":"Explicit matching tools (profile/target in identities); no Rust build during the proof. Export plumbing and preactivation validation, not native preview, clean-machine operation or full release acceptance."}
    (output / "new-story-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Starter created, checked, both branches completed and exported; player SHA-256: {player_hash}")
    print(f"Report and generated game: {output}")


if __name__ == "__main__":
    main()
