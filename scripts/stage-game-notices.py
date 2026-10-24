#!/usr/bin/env python3
"""Attach matching toolkit notice evidence to an exported game candidate."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import stat

spec = importlib.util.spec_from_file_location("transport", Path(__file__).with_name("package-folder.py"))
transport = importlib.util.module_from_spec(spec)
spec.loader.exec_module(transport)
require = transport.require
ROOT_FILES = ("LICENSE-MIT", "LICENSE-APACHE", "THIRD-PARTY-NOTICES.txt", "dependency-notices.json", "WINDOWS-REQUIREMENTS.txt")
MAX_DOCUMENT_BYTES = 4 * 1024 * 1024


def read_json(path):
    info = transport.plain_stat(path)
    require(stat.S_ISREG(info.st_mode) and info.st_size <= MAX_DOCUMENT_BYTES, "invalid metadata size/type")
    with path.open("rb") as source:
        data = source.read(MAX_DOCUMENT_BYTES + 1)
    require(len(data) <= MAX_DOCUMENT_BYTES, "metadata grew beyond limit")
    result = json.loads(data)
    require(isinstance(result, dict), "metadata must be an object")
    return result


def snapshot(toolkit):
    records = transport.inventory(toolkit / "licenses")
    for name in ROOT_FILES:
        path = toolkit / name
        info = transport.plain_stat(path)
        require(stat.S_ISREG(info.st_mode) and info.st_size <= MAX_DOCUMENT_BYTES, "invalid notice size/type")
        records[name] = {"size": info.st_size, "sha256": transport.file_hash(path, info.st_size)}
    require(len(records) <= transport.MAX_ENTRIES and sum(row.get("size", 0) for row in records.values()) <= transport.MAX_BYTES,
            "notice inventory exceeds bounds")
    return records


def stage(toolkit, game):
    for folder in (toolkit, game):
        require(stat.S_ISDIR(transport.plain_stat(folder).st_mode), "toolkit/game must be plain directories")
    toolkit, game = toolkit.resolve(strict=True), game.resolve(strict=True)
    require(not toolkit.is_relative_to(game) and not game.is_relative_to(toolkit), "toolkit/game overlap")
    output = game / "notices"
    require(not os.path.lexists(output), "game notices destination must be fresh")
    approved = snapshot(toolkit)
    report = read_json(toolkit / "dependency-notices.json")
    export = read_json(game / "weftora-export.json")
    require(report.get("auditApproved") is False, "only unapproved verification candidates are supported")
    lock = Path(__file__).resolve().parents[1] / "Cargo.lock"
    require(report["cargoLockSha256"] == transport.file_hash(lock), "toolkit lockfile differs from checkout")
    require(type(export.get("exportFormat")) is int and export["exportFormat"] == 1, "unsupported export format")
    require(export["player"]["identity"] == report["playerIdentity"]
            and export["cli"]["identity"] == report["cliIdentity"]
            and export["cli"]["sha256"] == report["cliSha256"], "export/toolkit identities differ")
    name = export["player"]["file"]
    require(name in ("weftora-player", "weftora-player.exe"), "unexpected player filename")
    player = game / name
    info = transport.plain_stat(player)
    require(stat.S_ISREG(info.st_mode) and info.st_size <= transport.MAX_BYTES, "invalid player size/type")
    player_hash = transport.file_hash(player, info.st_size)
    require(player_hash == export["player"]["sha256"] == report["playerSha256"], "game/toolkit player hashes differ")
    expected = {}
    for package in report["packages"]:
        for file in package["files"]:
            require(isinstance(file["path"], str), "notice path must be a string")
            path = transport.safe_name(file["path"])
            require(path.startswith("licenses/") and path not in expected, "invalid/duplicate notice path")
            expected[path] = file["sha256"]
    actual = {path: row["sha256"] for path, row in approved.items()
              if path.startswith("licenses/") and not row.get("directory")}
    require(actual == expected, "toolkit licence inventory/hash mismatch")
    require(snapshot(toolkit) == approved, "toolkit notices changed before copy")
    output.mkdir(exist_ok=False)
    for name, row in approved.items():
        destination = output / name
        if row.get("directory"):
            destination.mkdir(parents=True, exist_ok=True)
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            with (toolkit / name).open("rb") as source, destination.open("xb") as target:
                remaining = row["size"]
                while remaining:
                    chunk = source.read(min(transport.CHUNK, remaining))
                    require(chunk, "notice shrank during copy")
                    target.write(chunk)
                    remaining -= len(chunk)
                require(not source.read(1), "notice grew during copy")
            destination.chmod(0o755 if row.get("executable") else 0o644)
            require(transport.file_hash(destination, row["size"]) == row["sha256"], "copied notice hash mismatch")
    require(snapshot(toolkit) == approved and snapshot(output) == approved, "notice inventory changed during staging")
    require(transport.file_hash(player, info.st_size) == player_hash, "game player changed during staging")
    proof = {"playerSha256": player_hash, "toolkitReportSha256": approved["dependency-notices.json"]["sha256"],
             "exportMetadataSha256": transport.file_hash(game / "weftora-export.json"), "auditApproved": False,
             "scope": "Conservative matching authoring-tool notice superset; no compiler copied. Story/media terms, missing texts, native libraries and final audit remain separate."}
    (output / "notice-provenance.json").write_bytes((json.dumps(proof, indent=2) + "\n").encode("utf-8"))
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolkit", required=True, type=Path)
    parser.add_argument("--game", required=True, type=Path)
    args = parser.parse_args()
    try:
        print(stage(args.toolkit, args.game))
    except (ValueError, OSError, KeyError, TypeError) as error:
        parser.exit(1, f"game notice staging refused: {error}\n")


if __name__ == "__main__":
    main()
