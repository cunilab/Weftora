#!/usr/bin/env python3
"""Prove six content-only mechanics edits with one existing CLI/player pair."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

spec = importlib.util.spec_from_file_location("transport", Path(__file__).with_name("package-folder.py"))
transport = importlib.util.module_from_spec(spec)
spec.loader.exec_module(transport)
require, digest, inventory = transport.require, transport.file_hash, transport.inventory
CASES = ("rules", "commands", "collections", "scripts", "random", "queries")
CHECKPOINTS = {
    "rules": (("rules.start", 0),), "commands": (("commands.start", 0), ("commands.next", 1)),
    "collections": (("collections.start", 0), ("collections.next", 1)),
    "scripts": (("scripts.start", 0), ("scripts.next", 1)),
    "random": (("random.start", 0), ("random.second", 1), ("random.third", 2)),
    "queries": (("queries.start", 0), ("queries.next", 2)),
}


def expected_lines(name, edited):
    count = 3 if edited else 2
    return {
        "rules": ["First points 0", f"After rules {20 if edited else 10}"],
        "commands": [f"First points {4 if edited else 2}", f"Next points {8 if edited else 3}"],
        "collections": [f"Packed {count} items", "Items after reset 0"],
        "scripts": [f"Packed {count} items", "Items after reset 0"],
        "random": [f"Roll {value}" for value in ([6, 2, 1] if edited else [2, 1, 2])],
        "queries": [f"Packed {count} items", f"Now {count}, key true, schedule 3, morning true", "Items after reset 0"],
    }[name]


def edit_story(name, folder):
    before = inventory(folder)
    manifest_path = folder / "manifest.jsonc"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    require(manifest["saveSchema"] == 1, "review baseline save schema")
    changed = "events.jsonc"
    if name == "random":
        require(manifest["state"]["rng"]["default"] == 0, "review baseline seed")
        manifest["state"]["rng"]["default"] = 1
        changed = "manifest.jsonc"
    elif name == "scripts":
        path = folder / "mechanics.rhai"
        text = path.read_text(encoding="utf-8")
        require(text.count("let limit = amount;") == 1, "review baseline script limit")
        path.write_text(text.replace("let limit = amount;", "let limit = 3;"), encoding="utf-8")
        changed = "mechanics.rhai"
    else:
        path = folder / changed
        events = json.loads(path.read_text(encoding="utf-8"))
        if name == "rules":
            action = events["events"][0]["actions"][0]
            require(action == {"do": "state.add", "key": "points", "amount": 10}, "review baseline rule award")
            action["amount"] = 20
        elif name == "commands":
            action = events["commands"][1]["actions"][0]
            require(action == {"do": "state.add", "key": "points", "amount": {"argument": "n"}}, "review baseline increment")
            action["amount"] = 4
        else:
            require(name in ("collections", "queries"), "unknown mechanics case")
            actions = events["commands"][0]["actions"]
            require(actions[0] == {"do": "state.set", "key": "items", "value": {"literal": ["key", "ticket"]}}, "review baseline collection")
            actions[0]["value"]["literal"].append("pass")
            if name == "collections":
                require(actions[2] == {"do": "state.set", "key": "item_count", "value": 2}, "review baseline collection mirror")
                actions[2]["value"] = 3
            else:
                require("item_count" not in manifest["state"], "query fixture must not have a count mirror")
        path.write_text(json.dumps(events, indent=2) + "\n", encoding="utf-8")
    manifest["saveSchema"] = 2
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    after = inventory(folder)
    require(before.keys() == after.keys() and {key for key in before if before[key] != after[key]}
            == {folder.name + "/manifest.jsonc", folder.name + "/" + changed}, "unexpected mechanics edit files")


def check_state(name, edited, state, checkpoint=None):
    if name == "rules":
        # Rule feedback settles after the frozen line text, before the CLI saves.
        require(state["points"] == (20 if edited else 10) and state["awarded"] is True, "rule state mismatch")
    elif name == "commands":
        points = (4 if edited else 2) if checkpoint == "commands.start" else (8 if edited else 3)
        require(state["points"] == points, "command state mismatch")
    elif name == "random":
        rolls = [6, 2, 1] if edited else [2, 1, 2]
        count = dict(CHECKPOINTS[name])[checkpoint] + 1 if checkpoint else 3
        require(state["rolls"] == rolls[:count] and state["last"] == rolls[count - 1], "random state mismatch")
    else:
        packed = checkpoint == name + ".start"
        count = 3 if edited else 2
        require(state["items"] == (["key", "ticket", "pass"][:count] if packed else []), "collection items mismatch")
        schedule = {"morning": "station", "energy": 10 - count} if name == "scripts" else {
            "morning": "station", "energy": 3, "argument": "literal map entry"}
        require(state["schedule"] == (schedule if packed else {}), "collection schedule mismatch")
        if name == "queries":
            require("item_count" not in state, "query introduced a count mirror")
        else:
            require(state["item_count"] == (count if packed else 0), "collection count mismatch")


def prove(cli, player, output):
    repository = Path(__file__).resolve().parents[1]
    cli, player = cli.resolve(strict=True), player.resolve(strict=True)
    require(not os.path.lexists(output), "proof output must be fresh")
    output = output.resolve()
    require(not output.is_relative_to(repository), "proof output must be outside checkout")
    environment = dict(os.environ, WEFTORA_PLAYER=str(player))
    cli_hash, player_hash = digest(cli), digest(player)
    executable = "weftora-player.exe" if os.name == "nt" else "weftora-player"

    def invoke(*command):
        return subprocess.run([str(part) for part in command], env=environment, capture_output=True, encoding="utf-8", timeout=120)

    def run(*command):
        result = invoke(*command)
        require(result.returncode == 0, result.stderr or result.stdout or f"command failed: {command}")
        return result.stdout

    cli_id, player_id = json.loads(run(cli, "--tool-identity")), json.loads(run(player, "--tool-identity"))
    require(cli_id.get("tool") == "weftora" and player_id == dict(cli_id, tool="weftora-player"), "CLI/player identities differ")
    output.mkdir(parents=True, exist_ok=False)
    records = []
    for name in CASES:
        source = repository / "examples" / (name + "-demo")
        original = inventory(source)
        case = output / name
        case.mkdir()
        work = case / "working-story"
        shutil.copytree(source, work)
        variants, replays, refusals = [], [], []
        for edited in (False, True):
            if edited:
                edit_story(name, work)
            label = "edited" if edited else "baseline"
            destination = case / label
            run(cli, "export", work, "--out", destination)
            metadata = json.loads((destination / "weftora-export.json").read_text(encoding="utf-8"))
            exported_schema = json.loads((destination / "story/manifest.jsonc").read_text(encoding="utf-8"))["saveSchema"]
            require(exported_schema == (2 if edited else 1), "exported save schema mismatch")
            require(metadata["cli"]["identity"] == cli_id and metadata["player"]["identity"] == player_id
                    and metadata["cli"]["sha256"] == cli_hash and metadata["player"]["sha256"] == player_hash,
                    "export tool metadata mismatch")
            require(digest(destination / executable) == player_hash, "copied player hash mismatch")
            artifact = destination / "compiled/dialogue.json"
            require(metadata["dialogue"]["sha256"] == digest(artifact), "dialogue hash metadata mismatch")
            retained = case / "baseline" / executable
            run(retained, destination / "story", "--dialogue", artifact, "--validate")
            package_before = inventory(destination)

            def trace(*options):
                return run(cli, "run", destination / "story", "--headless", *options)

            full = trace()
            (case / (label + "-events.jsonl")).write_text(full, encoding="utf-8")
            events = [json.loads(line) for line in full.splitlines()]
            require(events[-1]["event"] == "end", "mechanics run did not complete")
            lines = [event["text"] for event in events if event["event"] == "line"]
            require(lines == expected_lines(name, edited), f"{name}/{label}: unexpected dialogue {lines}")
            final = events[-1]["state"]
            check_state(name, edited, final)
            for checkpoint, line_index in CHECKPOINTS[name]:
                saves = case / (label + "-" + checkpoint + "-saves")
                save_command = ("--save-at", checkpoint, "--save-dir", saves)
                for _ in range(2):
                    saved = [json.loads(line) for line in trace(*save_command).splitlines()]
                    require(saved[-1]["event"] == "saved", "checkpoint was not saved")
                check_state(name, edited, saved[-1]["state"], checkpoint)
                require((saves / "save.bak").is_file(), "second save did not preserve backup")
                slot = (saves / "save.json").read_bytes()
                resaved = [json.loads(line) for line in trace("--load", *save_command).splitlines()]
                require(resaved[-1]["event"] == "saved" and (saves / "save.json").read_bytes() == slot,
                        "checkpoint resave changed bytes")
                before = inventory(saves)
                loaded = [json.loads(line) for line in trace("--load", "--save-dir", saves).splitlines()]
                require(loaded[-1]["event"] == "end" and loaded[-1]["state"] == final, "restored final state differs")
                require([row["text"] for row in loaded if row["event"] == "line"] == lines[line_index:], "restored line sequence differs")
                require(inventory(saves) == before, "load changed save/backup files")
                replays.append({"variant": label, "checkpoint": checkpoint, "resaveByteIdentical": True,
                                "continuationMatches": True, "stateAtDisplay": saved[-1]["state"]})
            require(inventory(destination) == package_before, "replay changed exported package")
            variants.append({"variant": label, "saveSchema": exported_schema, "playerSha256": player_hash,
                             "compiledDialogueSha256": digest(artifact), "lines": lines, "finalState": final})
        require(variants[0]["compiledDialogueSha256"] == variants[1]["compiledDialogueSha256"], "mechanics edit changed compiled dialogue")
        edited_story = case / "edited/story"
        edited_before = inventory(case / "edited")
        for checkpoint, _ in CHECKPOINTS[name]:
            saves = case / ("baseline-" + checkpoint + "-saves")
            before = inventory(saves)
            failed = invoke(cli, "run", edited_story, "--headless", "--load", "--save-dir", saves)
            require(failed.returncode == 1 and not failed.stdout and "no save migration from schema 1 to 2" in failed.stderr,
                    f"wrong old-schema refusal: {failed.stderr}")
            require(inventory(saves) == before and inventory(case / "edited") == edited_before, "refusal changed save/package")
            (case / (checkpoint + "-schema-refusal.log")).write_text(failed.stderr, encoding="utf-8")
            refusals.append({"checkpoint": checkpoint, "exitCode": 1, "stdoutEmpty": True, "packageSlotAndBackupUnchanged": True})
        require(inventory(source) == original, "proof changed repository fixture")
        for label in ("baseline", "edited"):
            copied = case / label / executable
            require(digest(copied) == player_hash, "player changed before cleanup")
            copied.unlink()
        records.append({"case": name, "variants": variants, "checkpointReplays": replays, "oldSchemaRefusals": refusals})
    require(digest(cli) == cli_hash and digest(player) == player_hash, "proof input tools changed")
    report = {"cliIdentity": cli_id, "playerIdentity": player_id, "cliSha256": cli_hash, "playerSha256": player_hash,
              "platform": platform.platform(), "cases": records,
              "scope": "Actual exports, retained-player artifact validation and CLI headless mechanics/replay. No Rust build or native rendering/input acceptance; final P6-03 reconciliation remains separate."}
    (output / "same-player-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--player", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        report = prove(args.cli, args.player, args.out)
        print(f"Verified {len(report['cases'])} mechanics edits; unchanged player SHA-256: {report['playerSha256']}")
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"mechanics proof failed: {error}\n")


if __name__ == "__main__":
    main()
