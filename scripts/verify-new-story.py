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


HELLO_OLD = "Welcome to Weftora."
HELLO_NEW = "Welcome from the unchanged player."


def edit_hello(story):
    before = inventory(story)
    path = story / "dialogue/intro.yarn"
    text = path.read_text(encoding="utf-8")
    require(text.count(HELLO_OLD) == 1 and HELLO_NEW not in text, "review baseline Hello Story greeting")
    path.write_text(text.replace(HELLO_OLD, HELLO_NEW), encoding="utf-8")
    after = inventory(story)
    require(before.keys() == after.keys() and {key for key in before if before[key] != after[key]}
            == {"dialogue/intro.yarn"}, "Hello edit changed unexpected files")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--player", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    cli, player = args.cli.resolve(strict=True), args.player.resolve(strict=True)
    repository = Path(__file__).resolve().parents[1]
    require(not os.path.lexists(args.out), "proof output must be fresh")
    output = args.out.resolve()
    require(not output.is_relative_to(repository), "proof output must be outside checkout")
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
    hello = repository / "examples/hello-story"
    hello_before = inventory(hello)
    work = output / "hello-working"
    shutil.copytree(hello, work)
    hello_variants, hello_branches = [], {}
    for edited in (False, True):
        if edited:
            edit_hello(work)
        label = "hello-edited" if edited else "hello-baseline"
        destination = output / label
        run(cli, "export", work, "--out", destination)
        meta = json.loads((destination / "weftora-export.json").read_text(encoding="utf-8"))
        copied = destination / executable.name
        artifact = destination / "compiled/dialogue.json"
        require(meta["cli"]["identity"] == cli_identity and meta["player"]["identity"] == player_identity
                and meta["cli"]["sha256"] == cli_hash and meta["player"]["sha256"] == digest(copied) == player_hash,
                "Hello export tool identity/hash mismatch")
        require(meta["dialogue"]["sha256"] == digest(artifact), "Hello dialogue hash metadata mismatch")
        run(executable, destination / "story", "--dialogue", artifact, "--validate")
        for choice in (0, 1):
            trace = run(cli, "run", destination / "story", "--headless", "--choices", choice)
            (output / f"{label}-branch-{choice}.jsonl").write_text(trace, encoding="utf-8")
            events = [json.loads(line) for line in trace.splitlines()]
            require(events[-1]["event"] == "end" and events[-1]["state"]["curious"] is (choice == 0), "Hello branch state mismatch")
            if not edited:
                require(sum(event.get("text", "").count(HELLO_OLD) for event in events) == 1, "Hello baseline greeting missing")
                hello_branches[choice] = events
            else:
                expected_events = [dict(event) for event in hello_branches[choice]]
                for event in expected_events:
                    if event["event"] == "line":
                        event["text"] = event["text"].replace(HELLO_OLD, HELLO_NEW)
                require(events == expected_events, "Hello edit changed more than the greeting")
        require(inventory(destination / "story") == inventory(work), "Hello export changed authored content")
        hello_variants.append({"variant": label, "playerSha256": digest(copied), "compiledDialogueSha256": digest(artifact)})
        require(digest(copied) == player_hash, "Hello player changed before cleanup")
        copied.unlink()
    require(hello_variants[0]["compiledDialogueSha256"] != hello_variants[1]["compiledDialogueSha256"], "Hello text edit did not change compiled dialogue")
    require(inventory(hello) == hello_before and inventory(game / "story") == before, "Hello proof changed source/starter")
    require(digest(executable) == player_hash, "retained starter player changed")
    require(digest(cli) == cli_hash and digest(player) == player_hash, "proof input tools changed")
    shutil.copy2(cli, output / cli.name)
    require(digest(output / cli.name) == cli_hash, "retained CLI changed")
    report = {"check":"P6-01 automated starter workflow", "platform":platform.platform(),
              "cliSha256":cli_hash, "playerSha256":player_hash, "cliIdentity":cli_identity, "playerIdentity":player_identity, "generatedFiles":before,
              "branches":branches, "existingProjectRefusedWithoutWrites":True,
              "helloTextEdit": {"check":"P0-07 automated text-edit package/headless coverage", "variants":hello_variants,
                                "scope":"Same player bytes validate each artifact; gameplay traces use CLI headless. Profile/target inherit top-level identities; no rendered gameplay claim.",
                                "renderedOutput":False,
                                "bothBranchesDifferOnlyByGreeting":True, "sourceUnchanged":True},
              "sourceUnchanged":True, "exportedStoryMatchesSource":True, "standaloneValidationPassed":True,
              "scope":"Explicit matching tools (profile/target in identities); no Rust build during the proof. Starter/Hello text-edit exports and preactivation validation, not native preview, clean-machine operation or full release acceptance."}
    (output / "new-story-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Starter created, checked, both branches completed and exported; player SHA-256: {player_hash}")
    print(f"Report and generated game: {output}")


if __name__ == "__main__":
    main()
