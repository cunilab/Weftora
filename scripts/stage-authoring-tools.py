#!/usr/bin/env python3
"""Stage matching authoring tools and a dependency-notice review inventory."""
import argparse
import hashlib
import json
import os
import re
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



def upstream_notice_files(package, root, notice_root, index):
    matches = [entry for entry in index.get("packages", [])
               if (entry["name"], entry["version"]) == (package["name"], package["version"])]
    require(len(matches) <= 1, "duplicate upstream notice mapping")
    if not matches:
        require(not any(entry["name"] == package["name"] and any(
            file.get("kind") == "source-archive" for file in entry["files"])
            for entry in index.get("packages", [])), "source archive mapping missing for package version")
        return []
    entry = matches[0]
    require(entry["source"] == package.get("source") and entry["declaredLicense"] == package.get("license"),
            f"upstream notice metadata changed: {package['name']}")
    vcs = json.loads((root / ".cargo_vcs_info.json").read_text(encoding="utf-8"))
    require(not vcs["git"].get("dirty", False) and vcs["git"]["sha1"] == entry["upstreamCommit"], f"upstream notice revision changed: {package['name']}")
    result = []
    for file in entry["files"]:
        relative = Path(file["path"])
        require(not relative.is_absolute() and ".." not in relative.parts, "unsafe upstream notice path")
        path = notice_root / relative
        require(path.is_file() and not path.is_symlink() and path.resolve().is_relative_to(notice_root.resolve()),
                "upstream notice escapes checked-in collection")
        require(digest(path) == file["sha256"], f"upstream notice hash changed: {file['path']}")
        require(file.get("kind") in ("licence-text", "licensing-policy", "embedded-asset-notice", "source-archive"), "unknown upstream notice kind")
        result.append((path, "upstream/" + relative.as_posix(),
                       {"url": file["url"], "commit": entry["upstreamCommit"], "kind": file["kind"]}))
    return result


def check_archive_lock(package, lockfile, checksum):
    require(package.get("source") == "registry+https://github.com/rust-lang/crates.io-index",
            "source archives require a crates.io registry package")
    # Cargo-generated lock entries use quoted strings for these three fields.
    fields = {key: package[key] for key in ("name", "version", "source")}
    blocks = re.split(r"(?m)^\[\[package\]\]\s*$", lockfile.read_text(encoding="utf-8"))
    blocks = [re.split(r"(?m)^\[", block, maxsplit=1)[0] for block in blocks[1:]]
    matches = [block for block in blocks if all(
        re.search(r"(?m)^" + key + r"\s*=\s*" + re.escape(json.dumps(value)) + r"\s*$", block)
        for key, value in fields.items())]
    require(len(matches) == 1, "source archive package missing/ambiguous in Cargo.lock")
    sums = re.findall(r'(?m)^checksum\s*=\s*"([0-9a-f]{64})"\s*$', matches[0])
    require(sums == [checksum], "source archive checksum differs from Cargo.lock")


def collect_notices(metadata, repository, output):
    records = []
    notice_root = repository / "third-party" / "notices"
    index_file = notice_root / "index.json"
    index = json.loads(index_file.read_text(encoding="utf-8")) if index_file.is_file() else {"format": 1, "packages": []}
    require(index.get("format") == 1, "unsupported upstream notice index format")
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
        sources = [(path, path.name if workspace else path.relative_to(root).as_posix(), None) for path in files]
        if not workspace:
            sources += upstream_notice_files(package, root, notice_root, index)
        for path, _, provenance in sources:
            if provenance and provenance["kind"] == "source-archive":
                check_archive_lock(package, repository / "Cargo.lock", digest(path))
        for path, relative, provenance in sources:
            destination = output / "licenses" / (package["name"] + "-" + package["version"]) / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            require(not os.path.lexists(destination), "notice destination already exists")
            shutil.copy2(path, destination)
            if provenance and provenance["kind"] == "source-archive":
                check_archive_lock(package, repository / "Cargo.lock", digest(destination))
            file_record = {"path": destination.relative_to(output).as_posix(), "sha256": digest(destination)}
            if provenance:
                file_record["upstream"] = provenance
            record["files"].append(file_record)
        # A nested third-party notice alone cannot stand in for the crate's own licence.
        has_package_text = workspace or bool([path for path in files if path.parent == root]) or any(source[2] and source[2]["kind"] == "licence-text" for source in sources)
        record["missingTextOrMetadata"] = not record["declaredLicense"] or not has_package_text
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
             "Declared SPDX expressions are upstream metadata, not a legal determination. All available collected licence/notice texts are retained verbatim.",
             "Paths below are relative to the folder containing this notice.",
             "The Weftora player embeds Fira Sans Regular from Bevy's bevy_feathers assets, separately from bevy_text's Fira Mono default font.",
             "Both original font notices are retained under the bevy_text inventory entry as a collection anchor; this does not attribute Fira Sans to that crate.", ""]
    for record in records:
        lines.append(f"{record['name']} {record['version']}: {record['declaredLicense']}" + (" [MISSING TEXT OR LICENCE METADATA]" if record["missingTextOrMetadata"] else ""))
        lines.extend("  " + file["path"] for file in record["files"])
        for file in record["files"]:
            if file.get("upstream", {}).get("kind") == "source-archive":
                lines.extend([
                    f"  Source code for {record['name']} {record['version']} is included under {record['declaredLicense']}.",
                    "  Published source archive (gzip-compressed tar): " + file["path"],
                    "  SHA-256: " + file["sha256"],
                    "  Copy the archive to a separate folder and extract with tar -xf <archive>.",
                    "  The archive retains the upstream source, licence and attribution; see its licence file.",
                ])
    (output / "THIRD-PARTY-NOTICES.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    for source, name in (("authoring-README.txt", "README.txt"), ("windows-requirements.txt", "WINDOWS-REQUIREMENTS.txt")):
        shutil.copyfile(repository / "scripts" / "templates" / source, output / name)
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
