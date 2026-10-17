#!/usr/bin/env python3
"""Export Story A, Story B and a mechanics edit with one existing player."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--player", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    cli, player, output = args.cli.resolve(strict=True), args.player.resolve(strict=True), args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    repository = Path(__file__).resolve().parents[1]
    environment = dict(os.environ, WEFTORA_PLAYER=str(player))
    executable = "weftora-player.exe" if os.name == "nt" else "weftora-player"
    player_hash = digest(player)

    def invoke(*command):
        return subprocess.run([str(part) for part in command], env=environment,
                              capture_output=True, encoding="utf-8", timeout=120)

    def run(*command):
        result = invoke(*command)
        require(result.returncode == 0, result.stderr or result.stdout or f"failed: {command}")
        return result.stdout

    work = output / "working-edit-pay"
    shutil.copytree(repository / "examples" / "life-sim", work)
    records = []
    for name, source, choices in [("story-a", repository / "examples" / "story-a", "0,0"),
                                  ("story-b", repository / "examples" / "life-sim", "0,0,0,0"), ("edit-pay", work, "0,0,0,0")]:
        if name == "edit-pay":
            path = work / "mechanics.rhai"
            script = path.read_text(encoding="utf-8")
            require(script.count("let base_pay = 3;") == 1, "review baseline pay fixture")
            path.write_text(script.replace("let base_pay = 3;", "let base_pay = 5;"), encoding="utf-8")
            path = work / "manifest.jsonc"
            manifest = path.read_text(encoding="utf-8")
            require(manifest.count('"saveSchema": 1,') == 1, "review baseline save schema fixture")
            path.write_text(manifest.replace('"saveSchema": 1,', '"saveSchema": 2,'), encoding="utf-8")
        destination = output / name
        run(cli, "export", source, "--out", destination)
        run(destination / executable, destination / "story", "--validate")
        require(digest(destination / executable) == player_hash == digest(player), "player changed during exports")
        # One retained player validates all three packages; gameplay runs in the shared headless runner.
        run(output / "story-a" / executable, destination / "story", "--validate")
        trace = run(cli, "run", destination / "story", "--headless", "--choices", choices)
        (output / f"{name}-events.jsonl").write_text(trace, encoding="utf-8")
        events = [json.loads(line) for line in trace.splitlines()]
        require(events[-1]["event"] == "end", f"{name} did not complete")
        state = events[-1]["state"]
        if name == "story-a":
            require(state["story.route"] == "repair" and state["story.signal"] is True, "Story A route changed")
        else:
            require(state["location"] == "Home" and state["hour"] == 14 and state["energy"] == 3, "Story B day changed")
            require(state["inventory"] == ["permit", "receipt"] and state["rng"] == -7046029254386353131, "inventory/RNG changed")
            expected_pay = 4 if name == "story-b" else 6
            require(state["earned"] == expected_pay and state["coins"] == expected_pay - 2, "pay edit not applied")
        records.append({"variant": name, "playerSha256": player_hash, "traceRunner": "weftora CLI headless",
                        "compiledDialogueSha256": digest(destination / "compiled" / "dialogue.json"),
                        "traceSha256": hashlib.sha256(trace.encode("utf-8")).hexdigest(), "finalState": state})
        if name != "story-a":
            (destination / executable).unlink()
    require(records[1]["compiledDialogueSha256"] == records[2]["compiledDialogueSha256"], "script edit changed dialogue")
    # Refusals must publish no candidate output and preserve both good slot and backup.
    refusals = []
    for name, source, checkpoint, choices, target in [
        ("a-in-b", "story-a", "scene.menu", None, "story-b"),
        ("b-in-a", "story-b", "day.shift", "0,0", "story-a"),
        ("old-schema", "story-b", "day.shift", "0,0", "edit-pay"),
    ]:
        saves = output / (name + "-saves")
        command = [cli, "run", output / source / "story", "--headless", "--save-at", checkpoint, "--save-dir", saves]
        if choices:
            command += ["--choices", choices]
        run(*command)
        run(*command)  # Keep a backup too, so a failed load must preserve both files.
        before = {file: digest(saves / file) for file in ["save.json", "save.bak"]}
        failed = invoke(cli, "run", output / target / "story", "--headless", "--load", "--save-dir", saves)
        require(failed.returncode != 0 and not failed.stdout, f"{name} must refuse without output")
        expected = "migration" if name == "old-schema" else "save belongs to another Story"
        require(expected in failed.stderr, f"{name} refused for the wrong reason: {failed.stderr}")
        require(before == {file: digest(saves / file) for file in before}, f"{name} changed a save file")
        refusals.append({"case": name, "refusedWithoutOutputOrFileChanges": True})
    shutil.copy2(cli, output / "story-a" / cli.name)
    shutil.copy2(repository / "examples" / "life-sim" / "README.md", output / "README.md")
    report = {"checks": ["P5-01", "P5-02 automated package/headless coverage", "P5-05 cross-Story refusal"],
              "platform": platform.platform(), "cliSha256": digest(cli), "playerSha256": player_hash,
              "variants": records, "saveRefusals": refusals,
              "scope": "Package validation and shared-runner completion; no Rust build between exports. Native player completion pending issue #5. Phase 5 gate not claimed."}
    (output / "same-player-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Story A, Story B and edited pay validated; unchanged player SHA-256: {player_hash}")
    print(f"Report and packages: {output}")


if __name__ == "__main__":
    main()
