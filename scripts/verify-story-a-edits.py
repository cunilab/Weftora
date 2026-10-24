#!/usr/bin/env python3
"""Export two separate content edits with one existing player; never rebuild Rust."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--player", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    cli, player, output = args.cli.resolve(strict=True), args.player.resolve(strict=True), args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    source = Path(__file__).resolve().parents[1] / "examples" / "story-a"
    work = output / "working-story"
    shutil.copytree(source, work, symlinks=True)
    environment = dict(os.environ, WEFTORA_PLAYER=str(player))
    player_hash = digest(player)

    def run(*command):
        result = subprocess.run([str(part) for part in command], env=environment, capture_output=True, encoding="utf-8", timeout=120)
        if result.returncode:
            raise SystemExit(result.stderr or result.stdout or f"command failed: {command}")
        return result.stdout

    records = []
    test_saves = output / "compatibility-save"
    save_hash = None
    for name in ["baseline", "edit-dialogue-screen", "edit-theme-image"]:
        dialogue, screen = work / "main.yarn", work / "ui" / "dialogue.jsonc"
        if name == "edit-dialogue-screen":
            text = dialogue.read_text(encoding="utf-8")
            if "The ferry leaves at dawn." not in text:
                raise SystemExit("baseline dialogue fixture changed; review the edit proof")
            dialogue.write_text(text.replace("The ferry leaves at dawn.", "The ferry sails at sunrise."), encoding="utf-8")
            document = json.loads(screen.read_text(encoding="utf-8"))
            document["root"]["children"][0]["text"] = "LAST SIGNAL / EDIT 1"
            screen.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
        elif name == "edit-theme-image":
            document = json.loads(screen.read_text(encoding="utf-8"))
            panel = next(widget for widget in document["root"]["children"] if widget["type"] == "panel")
            panel["background"] = "#322033F0"
            screen.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
            shutil.copyfile(work / "assets" / "night.png", work / "assets" / "day.png")
        destination = output / name
        export_log = run(cli, "export", work, "--out", destination)
        exported_player = destination / ("weftora-player.exe" if os.name == "nt" else "weftora-player")
        validation = run(exported_player, destination / "story", "--validate")
        actual_hash = digest(exported_player)
        if actual_hash != player_hash or digest(player) != player_hash:
            raise SystemExit("player changed during content-only export")
        if name == "baseline":
            run(cli, "run", work, "--headless", "--save-at", "scene.arrival", "--save-dir", test_saves)
            save_hash = digest(test_saves / "save.json")
            compatibility = "baseline Arrival save created"
        else:
            trace = run(cli, "run", work, "--headless", "--load", "--choices", "0", "--save-dir", test_saves)
            events = [json.loads(line) for line in trace.splitlines()]
            first_line = next(event["text"] for event in events if event["event"] == "line")
            if "The ferry sails at sunrise." not in first_line or digest(test_saves / "save.json") != save_hash:
                raise SystemExit("text patch did not restore the original checkpoint without altering its save")
            compatibility = first_line
        records.append({
            "variant": name, "playerSha256": actual_hash,
            "dialogueSha256": digest(destination / "story" / "main.yarn"),
            "screenSha256": digest(destination / "story" / "ui" / "dialogue.jsonc"),
            "imageSha256": digest(destination / "story" / "assets" / "day.png"),
            "export": export_log.strip(), "validation": validation.strip(),
            "checkpointCompatibility": compatibility, "baselineSaveSha256": save_hash,
        })
        # Keep one standalone player plus both edited content packages, without duplicate binaries.
        if name != "baseline":
            exported_player.unlink()
    if (records[0]["dialogueSha256"] == records[1]["dialogueSha256"]
            or records[0]["screenSha256"] == records[1]["screenSha256"]
            or records[1]["screenSha256"] == records[2]["screenSha256"]
            or records[0]["imageSha256"] == records[2]["imageSha256"]):
        raise SystemExit("expected dialogue, screen/theme or image edit was not applied")
    shutil.copy2(cli, output / "baseline" / cli.name)
    shutil.copytree(source.parents[1] / "docs", output / "docs", symlinks=True)
    report = {"check": "P3-02", "cliSha256": digest(cli), "playerSha256": player_hash,
              "variants": records, "nativeVisualVerification": "pending maintainer in issue #5"}
    (output / "same-player-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Three validated Story A exports; unchanged player SHA-256: {player_hash}")
    print(f"Report and packages: {output}")


if __name__ == "__main__":
    main()
