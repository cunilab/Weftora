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
    cli_hash = digest(cli)

    def invoke(*command):
        return subprocess.run([str(part) for part in command], env=environment,
                              capture_output=True, encoding="utf-8", timeout=120)

    def run(*command):
        result = invoke(*command)
        require(result.returncode == 0, result.stderr or result.stdout or f"failed: {command}")
        return result.stdout

    cli_identity = json.loads(run(cli, "--tool-identity"))
    player_identity = json.loads(run(player, "--tool-identity"))
    require(player_identity == dict(cli_identity, tool="weftora-player"), "CLI/player identity mismatch")

    def verify_metadata(destination):
        metadata = json.loads((destination / "weftora-export.json").read_text(encoding="utf-8"))
        require(metadata["cli"]["identity"] == cli_identity and metadata["player"]["identity"] == player_identity,
                "export identity metadata mismatch")
        require(metadata["cli"]["sha256"] == cli_hash and metadata["player"]["sha256"] == player_hash,
                "export tool hash metadata mismatch")

    def inventory(source):
        return {p.relative_to(source).as_posix(): None if p.is_dir() else digest(p) for p in sorted(source.rglob("*"))}

    wrong_tools = []
    for name, source in [("story-a", repository / "examples/story-a"), ("story-b", repository / "examples/life-sim")]:
        before = inventory(source)
        for existing in [False, True]:
            destination = output / f"{name}-wrong-tool-{existing}"
            if existing:
                destination.mkdir()
            result = subprocess.run([str(cli), "export", str(source), "--out", str(destination)],
                                    env=dict(environment, WEFTORA_PLAYER=str(cli)), capture_output=True,
                                    encoding="utf-8", timeout=30)
            require(result.returncode == 1 and not result.stdout and "mismatched CLI/compiler/player" in result.stderr,
                    f"{name}: wrong executable role was not refused")
            require((destination.is_dir() and not any(destination.iterdir())) if existing else not destination.exists(),
                    "wrong-tool refusal changed output")
            require(inventory(source) == before, "wrong-tool refusal changed source")
            (output / f"{name}-wrong-tool-{existing}.log").write_text(result.stderr, encoding="utf-8")
            wrong_tools.append({"story":name, "existingEmptyOutput":existing, "exitCode":1, "stdoutEmpty":True,
                                "sourceAndOutputUnchanged":True, "actualTool":"weftora", "expectedTool":"weftora-player"})

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
        verify_metadata(destination)
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
    compatible = []
    downgrade_checks = []
    future_formats = []
    retained_player = output / "story-a" / executable
    for name, checkpoint, remaining, text in [
        ("story-a", "scene.repair", None, "Hold these two wires together. I will tell you when to let go."),
        ("story-b", "day.shift", "0,0", "{$status} Earned {$earned} coins."),
    ]:
        source = output / name / "story"
        saves = output / (name + "-compatible-saves")
        save_command = [cli, "run", source, "--headless", "--choices", "0,0", "--save-at", checkpoint, "--save-dir", saves]
        for _ in range(2):
            saved = [json.loads(line) for line in run(*save_command).splitlines()]
            require(saved[-1]["event"] == "saved", f"{name}: baseline checkpoint was not saved")
        before = {path.name: digest(path) for path in saves.iterdir()}
        revision = output / ("working-compatible-" + name)
        shutil.copytree(source, revision)
        path = revision / "manifest.jsonc"
        manifest = path.read_text(encoding="utf-8")
        require(manifest.count('"storyVersion": "0.1.0"') == 1 and manifest.count('"state": {') == 1,
                f"{name}: review compatible manifest fixture")
        manifest = manifest.replace('"storyVersion": "0.1.0"', '"storyVersion": "0.1.1"')
        manifest = manifest.replace('"state": {', '"state": {\n    "matrix_added": {"default": 23},')
        path.write_text(manifest, encoding="utf-8")
        path = revision / "main.yarn"
        dialogue = path.read_text(encoding="utf-8")
        require(dialogue.count(text) == 1, f"{name}: ambiguous text patch")
        path.write_text(dialogue.replace(text, "Compatibility patch is active. " + text), encoding="utf-8")
        # Story B has no authored media; its image-patch boundary is covered by the renderer fixture.
        asset_changed = name == "story-a"
        if asset_changed:
            require(digest(revision / "assets/night.png") != digest(revision / "assets/day.png"), "image patch needs different bytes")
            shutil.copy2(revision / "assets/day.png", revision / "assets/night.png")
        destination = output / ("compatible-" + name)
        run(cli, "export", revision, "--out", destination)
        verify_metadata(destination)
        if asset_changed:
            require(digest(destination / "story/assets/night.png") == digest(revision / "assets/day.png"),
                    "export did not include edited image bytes")
        require(digest(destination / executable) == player_hash == digest(retained_player) == digest(player), "compatible export rebuilt/replaced player")
        run(retained_player, destination / "story", "--validate")
        command = [cli, "run", destination / "story", "--headless", "--load", "--save-dir", saves]
        if remaining:
            command += ["--choices", remaining]
        trace = run(*command)
        (output / (name + "-compatible-events.jsonl")).write_text(trace, encoding="utf-8")
        events = [json.loads(line) for line in trace.splitlines()]
        require(any(event["event"] == "line" and "Compatibility patch is active." in event["text"] for event in events),
                f"{name}: restored dialogue did not use patched text")
        baseline = next(record["finalState"] for record in records if record["variant"] == name)
        require(events[-1]["event"] == "end" and events[-1]["state"] == dict(baseline, matrix_added=23),
                f"{name}: compatible load changed mechanics or dropped new defaults")
        require(before == {path.name: digest(path) for path in saves.iterdir()}, f"{name}: compatible load rewrote save directory")
        # Construct valid checksummed writer metadata; no historical Engine is executed.
        package_before = inventory(destination / "story")
        baseline_save = json.loads((saves / "save.json").read_text(encoding="utf-8"))
        require(baseline_save["body"]["writerEngineApi"] == "0.1.0", "review newer-writer API fixture")
        for case, patch, expected_error in [
            ("engine-api-downgrade", {"writerEngineApi":"0.1.1"}, "save was written by newer Engine API 0.1.1"),
            ("story-schema-downgrade", {"saveSchema":2, "storyVersion":"0.2.0"}, "save schema downgrade from 2 to 1 is unsupported"),
            ("compatible-release-rollback", {"storyVersion":"99.0.0", "writerEngineRelease":"99.0.0"}, None),
        ]:
            case_saves = output / f"{name}-{case}-saves"
            shutil.copytree(saves, case_saves)
            envelope = json.loads((case_saves / "save.json").read_text(encoding="utf-8"))
            envelope["body"].update(patch)
            body = json.dumps(envelope["body"], ensure_ascii=False, separators=(",", ":"), allow_nan=False)
            body_hash = hashlib.sha256(b"weftora-save-format-1\0" + body.encode("utf-8")).hexdigest()
            encoded = '{"saveFormat":1,"bodySha256":"' + body_hash + '","body":' + body + '}\n'
            (case_saves / "save.json").write_text(encoded, encoding="utf-8")
            case_before = inventory(case_saves)
            command = [cli, "run", destination / "story", "--headless", "--load", "--save-dir", case_saves]
            if remaining:
                command += ["--choices", remaining]
            result = invoke(*command)
            if expected_error:
                require(result.returncode == 1 and not result.stdout and expected_error in result.stderr,
                        f"{name}/{case}: wrong downgrade refusal: {result.stderr}")
                (output / f"{name}-{case}.log").write_text(result.stderr, encoding="utf-8")
            else:
                require(result.returncode == 0, result.stderr)
                loaded = [json.loads(line) for line in result.stdout.splitlines()]
                require(loaded[-1]["event"] == "end" and loaded[-1]["state"] == dict(baseline, matrix_added=23),
                        f"{name}: compatible release rollback changed mechanics")
                (output / f"{name}-{case}.jsonl").write_text(result.stdout, encoding="utf-8")
            require(inventory(case_saves) == case_before and inventory(destination / "story") == package_before,
                    f"{name}/{case}: load changed saves or package")
            downgrade_checks.append({"story":name, "case":case, "writerMetadataPatch":patch,
                                     "writerMetadataModeled":True, "readerEngineApi":"0.1.0",
                                     "exitCode":result.returncode, "sourceAndSaveDirectoryUnchanged":True,
                                     "saveBodySha256":body_hash, "checkpoint":checkpoint})
        compiled = destination / "compiled/dialogue.json"
        compiled_hash = digest(compiled)
        require(compiled_hash != digest(output / name / "compiled/dialogue.json"), "text patch did not change dialogue artifact")
        future = json.loads(compiled.read_text(encoding="utf-8"))
        future["dialogueArtifactFormat"] = 999
        future_path = output / (name + "-unsupported-dialogue.json")
        future_path.write_text(json.dumps(future), encoding="utf-8")
        failed = invoke(retained_player, destination / "story", "--dialogue", future_path, "--validate")
        require(failed.returncode == 1 and not failed.stdout and "unsupported dialogueArtifactFormat 999" in failed.stderr,
                f"{name}: future dialogue format was not refused specifically: {failed.stderr}")
        (output / (name + "-future-format.log")).write_text(failed.stderr, encoding="utf-8")
        require(digest(compiled) == compiled_hash, "negative artifact validation changed the good artifact")
        compatible.append({"story":name, "checkpoint":checkpoint, "storyVersion":"0.1.1", "saveSchemaUnchanged":True,
                           "playerSha256":player_hash, "authoredAssetPatched":asset_changed,
                           "assetVerification":"exported edited bytes; pixel use tested separately by renderer fixture" if asset_changed else "no authored media; renderer image-patch fixture only",
                           "oldSaveCompletedWithPatchedTextAndNewDefault":True, "saveDirectoryUnchanged":True,
                           "compiledDialogueSha256":compiled_hash, "finalState":events[-1]["state"]})
        future_formats.append({"story":name, "exitCode":1, "stdoutEmpty":True, "goodArtifactUnchanged":True})
        (destination / executable).unlink()
    require(digest(cli) == cli_hash and digest(player) == player_hash, "proof input tools changed")
    shutil.copy2(cli, output / "story-a" / cli.name)
    shutil.copy2(repository / "examples" / "life-sim" / "README.md", output / "README.md")
    report = {"checks": ["P5-01", "P5-02 automated package/headless coverage", "P5-05 cross-Story and compatible revisions"],
              "platform": platform.platform(), "cliSha256": digest(cli), "playerSha256": player_hash,
              "cliIdentity":cli_identity, "playerIdentity":player_identity, "wrongToolRefusals":wrong_tools,
              "downgradeCompatibility":downgrade_checks,
              "variants": records, "saveRefusals": refusals, "compatibleRevisions":compatible, "futureDialogueFormatRefusals":future_formats,
              "scope": "Package validation and shared-runner completion; no Rust build between exports. Native player completion pending issue #5. Phase 5 gate not claimed."}
    (output / "same-player-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Story A, Story B and edited pay validated; unchanged player SHA-256: {player_hash}")
    print(f"Report and packages: {output}")


if __name__ == "__main__":
    main()
