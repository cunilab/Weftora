#!/usr/bin/env python3
"""Validate P4-06 layout/theme/navigation edits with one prebuilt player."""
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
    cli, player = args.cli.resolve(strict=True), args.player.resolve(strict=True)
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    repository = Path(__file__).resolve().parents[1]
    environment = dict(os.environ, WEFTORA_PLAYER=str(player))
    player_hash = digest(player)
    executable = "weftora-player.exe" if os.name == "nt" else "weftora-player"

    def run(*command):
        result = subprocess.run([str(part) for part in command], env=environment,
                                capture_output=True, encoding="utf-8", timeout=120)
        if result.returncode:
            raise SystemExit(result.stderr or result.stdout or f"command failed: {command}")
        return result.stdout

    work = output / "working-story"
    shutil.copytree(repository / "examples" / "screens-demo", work)
    records = []
    for name, source in [("hello", repository / "examples" / "hello-story"),
                         ("baseline", work), ("edit-ui", work)]:
        if name == "edit-ui":
            path = work / "ui" / "menu.jsonc"
            document = json.loads(path.read_text(encoding="utf-8"))
            root = document["root"]
            root["background"] = "#302338"
            root["layout"]["padding"] = 48
            root["children"][0]["text"] = "STATION DESK / NIGHT"
            button = root["children"][4]["children"][0]
            require(button["action"]["screen"] == "activity", "review baseline navigation")
            button["text"] = "Open compact log"
            button["action"]["screen"] = "dialogue"
            path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
        destination = output / name
        run(cli, "export", source, "--out", destination)
        run(destination / executable, destination / "story", "--validate")
        actual_hash = digest(destination / executable)
        require(actual_hash == player_hash == digest(player), "player changed during exports")
        record = {"variant": name, "playerSha256": actual_hash}
        if name != "hello":
            # The retained baseline player must accept the edited package too.
            run(output / "baseline" / executable, destination / "story", "--validate")
            trace = run(cli, "run", destination / "story", "--headless", "--choices", "0")
            events = [json.loads(line) for line in trace.splitlines()]
            require(events[-1]["event"] == "end" and events[-1]["state"]["task"] == "Station shift",
                    "station branch did not complete")
            require(events[-1]["state"]["prepared"] is False, "headless play must not execute UI actions")
            record["lines"] = [event["text"] for event in events if event["event"] == "line"]
            record["headlessTraceSha256"] = hashlib.sha256(trace.encode("utf-8")).hexdigest()
            record["compiledDialogueSha256"] = digest(destination / "compiled" / "dialogue.json")
            record["screensSha256"] = {
                path.name: digest(path) for path in sorted((destination / "story" / "ui").glob("*.jsonc"))
            }
            menu = json.loads((destination / "story" / "ui" / "menu.jsonc").read_text(encoding="utf-8"))["root"]
            record["menu"] = {"background": menu["background"], "padding": menu["layout"]["padding"],
                              "firstTarget": menu["children"][4]["children"][0]["action"]["screen"]}
        records.append(record)
        if name != "baseline":
            (destination / executable).unlink()
    baseline, edited = records[1:]
    require(baseline["compiledDialogueSha256"] == edited["compiledDialogueSha256"], "compiled dialogue changed")
    require(baseline["headlessTraceSha256"] == edited["headlessTraceSha256"], "headless event trace changed")
    require(baseline["screensSha256"]["menu.jsonc"] != edited["screensSha256"]["menu.jsonc"], "menu edit missing")
    require(baseline["menu"] == {"background": "#0B1220", "padding": 32, "firstTarget": "activity"},
            "review baseline menu")
    require(edited["menu"] == {"background": "#302338", "padding": 48, "firstTarget": "dialogue"},
            "UI edit did not apply")
    for file in ["activity.jsonc", "dialogue.jsonc", "pause.jsonc"]:
        require(baseline["screensSha256"][file] == edited["screensSha256"][file], f"unintended screen edit: {file}")
    shutil.copy2(cli, output / "baseline" / cli.name)
    shutil.copy2(repository / "examples" / "screens-demo" / "README.md", output / "README.md")
    report = {"check": "P4-06", "platform": platform.platform(), "cliSha256": digest(cli),
              "playerSha256": player_hash, "variants": records,
              "scope": "Package validation and headless dialogue; no Rust build between exports. UI model tests run separately.",
              "nativeVisualVerification": "pending maintainer in issue #5"}
    (output / "same-player-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Three validated exports; unchanged player SHA-256: {player_hash}")
    print(f"Report and packages: {output}")


if __name__ == "__main__":
    main()
