#!/usr/bin/env python3
"""Stage matching authoring tools and a dependency-notice review inventory."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import unicodedata

LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def tool_identity(path):
    result = subprocess.run([str(path), "--tool-identity"], capture_output=True, check=True, timeout=15)
    require(len(result.stdout) <= 8192, "tool identity exceeds 8 KiB")
    return json.loads(result.stdout)


def dependency_packages(metadata):
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    roots = [package["id"] for package in packages.values() if package["name"] in ("weftora-cli", "weftora-player")
             and package["id"] in metadata["workspace_members"]]
    require(len(roots) == 2, "metadata must contain both workspace authoring tools")
    visited, pending = set(), list(roots)
    while pending:
        package_id = pending.pop()
        if package_id in visited:
            continue
        visited.add(package_id)
        pending.extend(dependency["pkg"] for dependency in nodes[package_id]["deps"]
                       if any(kind["kind"] in (None, "build") for kind in dependency["dep_kinds"]))
    return sorted((packages[package_id] for package_id in visited), key=lambda package: (package["name"], package["version"]))


def licence_files(root, declared_file):
    root = root.resolve(strict=True)
    candidates = set()
    if declared_file:
        declared = Path(declared_file)
        candidates.add(declared if declared.is_absolute() else root / declared)
    for directory, directories, names in os.walk(root, followlinks=False):
        directories[:] = sorted(name for name in directories if name not in (".git", "target"))
        for name in names:
            if any(marker in name.upper() for marker in LICENSE_PREFIXES):
                candidates.add(Path(directory) / name)
    result = set()
    for path in sorted(candidates):
        require(path.is_file() and not path.is_symlink(), f"licence text is missing/not a plain file: {path}")
        require(path.resolve().is_relative_to(root.resolve()), f"licence path escapes package: {path}")
        require(path.stat().st_size <= 4 * 1024 * 1024, f"licence text exceeds 4 MiB: {path}")
        result.add(path.resolve(strict=True))
    return sorted(result)


def collect_notices(metadata, repository, output):
    records = []
    packages = dependency_packages(metadata)
    keys = [unicodedata.normalize("NFC", package["name"] + "-" + package["version"]).casefold() for package in packages]
    require(len(keys) == len(set(keys)), "ambiguous package name/version notice destination")
    for package in packages:
        root = Path(package["manifest_path"]).parent.resolve(strict=True)
        workspace = package["id"] in metadata["workspace_members"]
        files = [repository / "LICENSE-MIT", repository / "LICENSE-APACHE"] if workspace else licence_files(root, package.get("license_file"))
        record = {"name": package["name"], "version": package["version"], "declaredLicense": package.get("license"),
                  "repository": package.get("repository"), "workspace": workspace,
                  "source": "workspace" if workspace else (package.get("source") or "vendored path dependency"), "files": []}
        for path in files:
            relative = path.name if workspace else path.relative_to(root).as_posix()
            destination = output / "licenses" / (package["name"] + "-" + package["version"]) / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, destination)
            record["files"].append({"path": destination.relative_to(output).as_posix(), "sha256": digest(destination)})
        record["missingTextOrMetadata"] = not record["declaredLicense"] or not record["files"]
        records.append(record)
    return records


def stage(cli, player, output):
    repository = Path(__file__).resolve().parents[1]
    cli, player = cli.resolve(strict=True), player.resolve(strict=True)
    cli_hash, player_hash = digest(cli), digest(player)
    cli_id, player_id = tool_identity(cli), tool_identity(player)
    require(cli_id.get("tool") == "weftora" and player_id == dict(cli_id, tool="weftora-player"), "CLI/player identities must match")
    require(cli.name == ("weftora.exe" if os.name == "nt" else "weftora")
            and player.name == ("weftora-player.exe" if os.name == "nt" else "weftora-player"), "unexpected tool filenames")
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", cli_id["target"]], cwd=repository))
    packages = dependency_packages(metadata)
    require(all(package["version"] == cli_id["engineRelease"] for package in packages if package["id"] in metadata["workspace_members"]),
            "tool release differs from workspace version")
    require(not os.path.lexists(output), "staging output must be fresh")
    output = output.resolve()
    require(not output.is_relative_to(repository), "staging output must be outside the checkout")
    output.mkdir(parents=True, exist_ok=False)
    for source, expected in [(cli, cli_hash), (player, player_hash)]:
        shutil.copy2(source, output / source.name)
        require(digest(source) == digest(output / source.name) == expected, "tool changed during staging")
    for name in ("LICENSE-MIT", "LICENSE-APACHE"):
        shutil.copy2(repository / name, output / name)
    records = collect_notices(metadata, repository, output)
    report = {"target": cli_id["target"], "profile": cli_id["profile"], "cliIdentity": cli_id, "playerIdentity": player_id,
              "cliSha256": cli_hash, "playerSha256": player_hash, "cargoLockSha256": digest(repository / "Cargo.lock"),
              "packages": records, "auditApproved": False,
              "scope": "Conservative workspace-feature normal/build dependency inventory for the tool target, not exact binary linkage. Copied available upstream texts; missing texts, embedded assets, external native libraries and licence obligations require review. Not release approval."}
    (output / "dependency-notices.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    lines = ["Third-party notice review inventory", "", "This candidate is not approved for release. See dependency-notices.json.",
             "Declared SPDX expressions are upstream metadata, not a legal determination. All available collected licence/notice texts are retained verbatim.", ""]
    for record in records:
        lines.append(f"{record['name']} {record['version']}: {record['declaredLicense']}" + (" [MISSING TEXT OR LICENCE METADATA]" if record["missingTextOrMetadata"] else ""))
        lines.extend("  " + file["path"] for file in record["files"])
    (output / "THIRD-PARTY-NOTICES.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    readme = (repository / "scripts" / "templates" / "authoring-README.txt").read_text(encoding="utf-8")
    (output / "README.txt").write_text(readme, encoding="utf-8")
    require(tool_identity(output / cli.name) == cli_id and tool_identity(output / player.name) == player_id, "staged tool identity changed")
    require(digest(cli) == cli_hash and digest(player) == player_hash, "original tools changed")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--player", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        report = stage(args.cli, args.player, args.out)
        missing = sum(record["missingTextOrMetadata"] for record in report["packages"])
        print(f"Staged {len(report['packages'])} packages; {missing} need missing-text/metadata review. Audit approval remains pending.")
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"staging refused: {error}\n")


if __name__ == "__main__":
    main()
