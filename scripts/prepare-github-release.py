#!/usr/bin/env python3
"""Verify frozen main CI assets; optionally create an unpublished GitHub Release draft."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import time
from urllib.error import HTTPError
from urllib.parse import quote, urlsplit
from urllib.request import build_opener, HTTPRedirectHandler, Request
import zipfile

try:
    import tomllib
except ImportError:
    tomllib = None

spec = importlib.util.spec_from_file_location("release_assets", Path(__file__).with_name("stage-release-assets.py"))
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)
require = assets.require
REPO = "cunilab/Weftora"
SHA = r"[0-9a-f]{40}"
HASH = r"[0-9a-f]{64}"
MIB = 1024 * 1024
BUILD_FILES = {"build-provenance.json", "source-files.json", "msvc-linker.txt", "msvc-compiler.txt", "release-tests.log", "release-build.log"}
ZIP_NAMES = {"weftora-authoring-tools.zip", *(f"weftora-transport-{story}.zip" for story in ("story-a", "life-sim", "localization-demo"))}
DOWNLOAD_FILES = ZIP_NAMES | {"release-assets.json", "SHA256SUMS"}
JOBS = {"Windows (required)", "Dependencies (required)", "Windows release verification"}


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, url):
        return None


class GitHub:
    def __init__(self):
        token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
        if not token:
            token = subprocess.check_output(["gh", "auth", "token"], text=True).strip()
        require(bool(token), "GitHub credentials are required")
        self.headers = {"Authorization": "Bearer " + token, "Accept": "application/vnd.github+json", "User-Agent": "weftora-release-preflight"}
        self.opener = build_opener(NoRedirect)

    def request(self, path):
        return Request("https://api.github.com/repos/" + REPO + "/" + path, headers=self.headers)

    def json(self, path):
        with self.opener.open(self.request(path), timeout=30) as response:
            data = response.read(8 * MIB + 1)
        require(len(data) <= 8 * MIB, "GitHub metadata exceeds limit")
        return json.loads(data)

    def pages(self, path, key=None):
        rows = []
        for page in range(1, 21):
            data = self.json(path + ("&" if "?" in path else "?") + f"per_page=100&page={page}")
            batch = data[key] if key else data
            require(isinstance(batch, list), "invalid GitHub listing")
            rows.extend(batch)
            if len(batch) < 100:
                return rows
        raise ValueError("GitHub listing exceeds 2000 entries")

    def download(self, artifact, destination, limit):
        size = artifact["size_in_bytes"]
        require(type(size) is int and 0 < size <= limit, "artifact download size exceeds limit")
        require(re.fullmatch("sha256:" + HASH, artifact.get("digest") or ""), "artifact has no SHA-256 digest")
        try:
            response = self.opener.open(self.request(f"actions/artifacts/{artifact['id']}/zip"), timeout=30)
        except HTTPError as error:
            require(error.code == 302, "artifact download did not return the expected redirect")
            location = error.headers.get("Location", "")
            error.close()
        else:
            response.close()
            raise ValueError("artifact download requires a signed redirect")
        url = urlsplit(location)
        require(url.scheme == "https" and url.hostname and not url.username and not url.password
                and (url.hostname.endswith(".blob.core.windows.net") or url.hostname.endswith(".actions.githubusercontent.com")),
                "unrecognized artifact download host")
        digest, count, deadline = hashlib.sha256(), 0, time.monotonic() + 600
        # The signed storage request deliberately carries no GitHub authentication header.
        with self.opener.open(Request(location, headers={"User-Agent": "weftora-release-preflight"}), timeout=30) as response, destination.open("xb") as output:
            for block in iter(lambda: response.read(MIB), b""):
                count += len(block)
                require(count <= size and time.monotonic() <= deadline, "artifact download exceeded size/time limit")
                digest.update(block)
                output.write(block)
        require(count == size and "sha256:" + digest.hexdigest() == artifact["digest"], "artifact container checksum mismatch")


def git(root, *arguments, binary=False, data=None):
    result = subprocess.run(["git", *arguments], cwd=root, input=data, capture_output=True, check=True, timeout=120)
    return result.stdout if binary else result.stdout.decode("utf-8").strip()


def release_version(tag):
    numeric = r"(?:0|[1-9][0-9]*)"
    match = re.fullmatch(r"v(" + numeric + r"\." + numeric + r"\." + numeric + r")(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?", tag)
    require(len(tag) <= 128 and match, "tag must be v-prefixed SemVer")
    pre = match[2]
    require(not pre or all(not p.isdigit() or len(p) == 1 or p[0] != "0" for p in pre.split(".")), "numeric prerelease identifier has a leading zero")
    return tag[1:], bool(pre)


def source_versions(root, commit, version):
    def toml(path):
        return tomllib.loads(git(root, "show", commit + ":" + path, binary=True).decode("utf-8"))
    workspace = toml("Cargo.toml")["workspace"]
    require(workspace["package"]["version"] == version, "tag and workspace version differ")
    lock = toml("Cargo.lock")["package"]
    names = set()
    for member in workspace["members"]:
        assets.transport.safe_name(member)
        package = toml(member + "/Cargo.toml")["package"]
        name = package["name"]
        require(name not in names, "duplicate workspace package")
        names.add(name)
        declared = package["version"]
        require(declared == version or declared == {"workspace": True}, "workspace member version differs")
        entries = [p for p in lock if p["name"] == name and "source" not in p]
        require(len(entries) == 1 and entries[0]["version"] == version, "workspace/lockfile versions differ")
    require(names, "workspace has no members")


def remote_state(root, client, tag, run_id, version):
    run = client.json(f"actions/runs/{run_id}")
    require(run["event"] == "push" and run["head_branch"] == "main" and run["path"] == ".github/workflows/ci.yml",
            "release requires a CI push run from main")
    require(run["repository"]["full_name"] == REPO and run["head_repository"]["full_name"] == REPO,
            "CI repository differs")
    require(run["status"] == "completed" and run["conclusion"] == "success", "CI run must be complete and successful")
    attempt = run["run_attempt"]
    require(type(attempt) is int and attempt > 0, "invalid CI attempt")
    ref = client.json("git/ref/tags/" + quote(tag, safe=""))["object"]
    require(ref["type"] == "tag" and re.fullmatch(SHA, ref["sha"]), "release tag must be annotated")
    annotated = client.json("git/tags/" + ref["sha"])["object"]
    require(annotated["type"] == "commit" and re.fullmatch(SHA, annotated["sha"]), "annotated tag must point directly to a commit")
    commit = annotated["sha"]
    require(git(root, "rev-parse", "refs/tags/" + tag) == ref["sha"], "local and remote tag objects differ")
    require(run["head_sha"] == commit, "CI and tag commit differ")
    main = client.json("git/ref/heads/main")["object"]
    require(main["type"] == "commit" and re.fullmatch(SHA, main["sha"]), "invalid main reference")
    # A stale local clone refuses; the operator fetches main/tags before retrying.
    git(root, "merge-base", "--is-ancestor", commit, main["sha"])
    source_versions(root, commit, version)
    jobs = client.pages(f"actions/runs/{run_id}/attempts/{attempt}/jobs", "jobs")
    for name in JOBS:
        matching = [j for j in jobs if j["name"] == name]
        require(len(matching) == 1 and matching[0]["status"] == "completed" and matching[0]["conclusion"] == "success",
                "required CI job missing/unsuccessful: " + name)
    require(not any(r["tag_name"] == tag for r in client.pages("releases")), "a draft/release already exists for this tag")
    found = client.pages(f"actions/runs/{run_id}/artifacts", "artifacts")
    selected = {}
    for kind in ("build-provenance", "download-candidate"):
        name = f"weftora-windows-{kind}-{commit}-attempt-{attempt}"
        matching = [a for a in found if a["name"] == name]
        require(len(matching) == 1, "missing/ambiguous attempt-qualified artifact: " + name)
        artifact = matching[0]
        require(artifact["expired"] is False and artifact["workflow_run"]["id"] == run_id
                and artifact["workflow_run"]["head_sha"] == commit, "expired or mismatched artifact")
        require(type(artifact["id"]) is int and artifact["id"] > 0, "invalid artifact ID")
        selected[kind] = artifact
    return {"commit": commit, "tagObject": ref["sha"], "attempt": attempt, "artifacts": selected}


def unpack(archive, destination, allowed, limit):
    with zipfile.ZipFile(archive) as z:
        entries = z.infolist()
        require(len(entries) == len(allowed) and {e.orig_filename for e in entries} == allowed,
                "artifact contains unexpected/duplicate files")
        require(sum(e.file_size for e in entries) <= limit, "artifact uncompressed bytes exceed limit")
        for entry in entries:
            require(entry.filename == entry.orig_filename and not entry.is_dir()
                    and stat.S_IFMT(entry.external_attr >> 16) in (0, stat.S_IFREG)
                    and not entry.flag_bits & 1 and entry.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED),
                    "unsupported artifact entry")
        destination.mkdir()
        for entry in entries:
            size = 0
            with z.open(entry) as source, (destination / entry.filename).open("xb") as output:
                for block in iter(lambda: source.read(MIB), b""):
                    size += len(block)
                    require(size <= entry.file_size, "artifact member exceeded size")
                    output.write(block)
            require(size == entry.file_size, "truncated artifact member")


def fingerprint(path):
    info = assets.transport.plain_stat(path)
    require(stat.S_ISREG(info.st_mode), "expected a plain release input")
    return {"bytes": info.st_size, "sha256": assets.transport.file_hash(path, info.st_size)}


def check_sources(root, commit, manifest):
    entries = git(root, "ls-tree", "-rz", "--full-tree", commit, binary=True).split(b"\0")[:-1]
    rows = []
    for entry in entries:
        header, name = entry.split(b"\t", 1)
        mode, kind, oid = header.decode().split()
        require(mode in ("100644", "100755") and kind == "blob", "unsupported source tree entry")
        rows.append((name.decode("utf-8"), oid))
    require(set(manifest) == {name for name, _ in rows}, "build source file set differs from Git")
    require(sum(row["bytes"] for row in manifest.values()) <= 256 * MIB, "source inventory exceeds limit")
    requests = ("\n".join(oid for _, oid in rows) + "\n").encode()
    sizes = git(root, "cat-file", "--batch-check", binary=True, data=requests).splitlines()
    require(len(sizes) == len(rows) and sum(int(line.split()[2]) for line in sizes) <= 256 * MIB, "Git source bytes exceed limit")
    data = git(root, "cat-file", "--batch", binary=True, data=requests)
    offset = 0
    for name, oid in rows:
        end = data.index(b"\n", offset)
        actual_oid, kind, size = data[offset:end].decode().split();size = int(size)
        require(actual_oid == oid and kind == "blob", "Git blob response mismatch")
        content = data[end + 1:end + 1 + size]
        require(len(content) == size and manifest[name] == {"bytes": size, "sha256": hashlib.sha256(content).hexdigest()},
                "build source bytes differ from Git: " + name)
        offset = end + 1 + size + 1
    require(offset == len(data), "unexpected Git blob output")


def check_payload(root, state, version, output):
    build_dir, downloads = output / "build", output / "downloads"
    build = json.loads((build_dir / "build-provenance.json").read_text(encoding="utf-8"))
    require(build["sourceCommit"] == state["commit"] and build["sourceTree"] == git(root, "rev-parse", state["commit"] + "^{tree}"), "build source revision differs")
    require(build["environment"]["GITHUB_RUN_ID"] == str(state["runId"]) and build["environment"]["GITHUB_RUN_ATTEMPT"] == str(state["attempt"]), "build run/attempt differs")
    require(build["target"] == "x86_64-pc-windows-msvc" and build["profile"] == "release" and build["freshTarget"] is True and build["auditApproved"] is False, "unsupported build candidate")
    require(set(build["evidenceFiles"]) == BUILD_FILES - {"build-provenance.json"}, "build evidence file set differs")
    for name, row in build["evidenceFiles"].items():
        require(fingerprint(build_dir / name) == row, "build evidence checksum mismatch")
    manifest = json.loads((build_dir / "source-files.json").read_text(encoding="utf-8"))
    check_sources(root, state["commit"], manifest)
    for name in ("Cargo.lock", "Cargo.toml", "rust-toolchain.toml", ".cargo/config.toml"):
        require(build["sourceInputs"][name] == manifest[name], "build input hash differs")
    require(set(build["binaries"]) == {"weftora.exe", "weftora-player.exe"}, "unexpected built binaries")
    records = assets.verify(downloads)
    require({r["name"] for r in records} == ZIP_NAMES, "expected four maintained release ZIPs")
    blocked = set()
    for row in records:
        with zipfile.ZipFile(downloads / row["name"]) as z:
            prefix = row["name"][:-4] + "/"
            toolkit = row["name"] == "weftora-authoring-tools.zip"
            notice_path = prefix + ("" if toolkit else "notices/") + "dependency-notices.json"
            notice_info = z.getinfo(notice_path)
            require(notice_info.file_size <= 8 * MIB, "notice metadata exceeds limit")
            notice = json.loads(z.read(notice_info))
            require(notice["target"] == build["target"] and notice["profile"] == "release" and notice["auditApproved"] is False, "notice target/profile differs")
            cli_id = notice["cliIdentity"]
            require(cli_id["engineRelease"] == version and notice["playerIdentity"] == dict(cli_id, tool="weftora-player")
                    and cli_id["tool"] == "weftora" and cli_id["target"] == build["target"] and cli_id["profile"] == "release", "tool identities/version differ")
            require(notice["cargoLockSha256"] == manifest["Cargo.lock"]["sha256"], "notice lockfile differs")
            require(notice["packages"], "empty notice inventory")
            for package in notice["packages"]:
                if package["missingTextOrMetadata"] is not False:
                    blocked.add(package["name"] + " " + package["version"])
            for role in ("weftora.exe", "weftora-player.exe"):
                key = "cliSha256" if role == "weftora.exe" else "playerSha256"
                require(notice[key] == build["binaries"][role]["sha256"], "notice binary checksum differs")
                if role == "weftora.exe" and not toolkit:
                    require(prefix + role not in z.namelist(), "game unexpectedly contains authoring CLI")
                    continue
                expected = build["binaries"][role];entry = z.getinfo(prefix + role)
                require(type(expected["bytes"]) is int and 0 < entry.file_size == expected["bytes"] <= assets.MAX_ASSET_BYTES, "packaged executable size differs")
                digest, count = hashlib.sha256(), 0
                with z.open(entry) as source:
                    for block in iter(lambda: source.read(MIB), b""):
                        count += len(block);require(count <= expected["bytes"], "packaged executable grew");digest.update(block)
                require(count == expected["bytes"] and digest.hexdigest() == expected["sha256"], "packaged executable checksum differs")
            if not toolkit:
                entry = z.getinfo(prefix + "weftora-export.json")
                require(entry.file_size <= 8 * MIB, "export metadata exceeds limit")
                export = json.loads(z.read(entry))
                require(export["cli"]["identity"] == cli_id and export["player"]["identity"] == notice["playerIdentity"]
                        and export["cli"]["sha256"] == notice["cliSha256"] and export["player"]["sha256"] == notice["playerSha256"], "export metadata differs")
    require(not blocked, "unresolved notice text/metadata: " + ", ".join(sorted(blocked)))
    return records, build["binaries"]


def prepare(root, client, tag, run_id, notes, output, create=False, reviewed=False):
    require(tomllib is not None, "draft preparation requires Python 3.11 or newer")
    version, prerelease = release_version(tag)
    require(type(run_id) is int and run_id > 0, "invalid CI run ID")
    require(not create or reviewed, "draft creation requires explicit completed-checklist attestation")
    require(not os.path.lexists(output), "release output must be fresh")
    root, output = root.resolve(strict=True), output.resolve()
    require("#" not in str(output), "output contains an asset-label separator")
    require(not output.is_relative_to(root) and not root.is_relative_to(output), "release output overlaps source checkout")
    info = assets.transport.plain_stat(notes)
    require(stat.S_ISREG(info.st_mode) and info.st_size <= 64 * 1024, "invalid release notes size/type")
    with notes.open("rb") as source:
        note_bytes = source.read(64 * 1024 + 1)
    require(len(note_bytes) <= 64 * 1024, "release notes exceed limit")
    note_text = note_bytes.decode("utf-8");require(note_text.strip(), "release notes are empty")
    state = remote_state(root, client, tag, run_id, version);state["runId"] = run_id
    output.mkdir(parents=True)
    for kind, allowed, limit in (("build-provenance", BUILD_FILES, 64 * MIB), ("download-candidate", DOWNLOAD_FILES, assets.MAX_TOTAL_BYTES + 256 * 1024)):
        archive = output / (kind + ".zip")
        client.download(state["artifacts"][kind], archive, limit + 16 * MIB)
        unpack(archive, output / ("build" if kind == "build-provenance" else "downloads"), allowed, limit)
    records, binaries = check_payload(root, state, version, output)
    names = sorted(ZIP_NAMES) + ["SHA256SUMS"]
    paths = [output / "downloads" / name for name in names]
    note_text += f"\n\nCandidate evidence\n\n- Source: `{state['commit']}`\n- CI: https://github.com/{REPO}/actions/runs/{run_id} (attempt {state['attempt']})\n- Target: `x86_64-pc-windows-msvc`, release profile\n- Download hashes: `SHA256SUMS`\n"
    for role, row in binaries.items():note_text += f"- {role} SHA-256: `{row['sha256']}`\n"
    note_copy = output / "release-notes.md";note_copy.write_text(note_text, encoding="utf-8")
    files = paths + [note_copy] + [output / "build" / name for name in BUILD_FILES] + [output / "downloads/release-assets.json"]
    snapshots = {str(path.relative_to(output)): fingerprint(path) for path in files}
    command = ["gh", "release", "create", tag, "--repo", REPO, "--verify-tag", "--draft", "--latest=false", "--title", "Weftora " + tag, "--notes-file", str(note_copy)]
    if prerelease:command.append("--prerelease")
    command += [str(path) for path in paths]
    plan = {"tag": tag, "sourceCommit": state["commit"], "runId": run_id, "runAttempt": state["attempt"], "artifacts": state["artifacts"], "assets": records, "binaries": binaries, "files": snapshots, "command": command, "checklistReviewed": reviewed, "draftCreated": False, "auditApproved": False, "scope": "Automatic source/CI/artifact/byte checks only. Native, licence and advisory acceptance require maintainer review; publication is separate."}
    (output / "draft-release-plan.tmp").write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
    (output / "draft-release-plan.tmp").rename(output / "draft-release-plan.json")
    if create:
        current = remote_state(root, client, tag, run_id, version);current["runId"] = run_id
        require(current == state, "remote release inputs changed after preflight")
        require(all(fingerprint(output / name) == row for name, row in snapshots.items()), "release input changed after preflight")
        subprocess.run(command, cwd=root, check=True, timeout=600)
        releases = [r for r in client.pages("releases") if r["tag_name"] == tag]
        require(len(releases) == 1, "draft creation readback missing/ambiguous")
        release = client.json(f"releases/{releases[0]['id']}")
        require(release["draft"] is True and release["prerelease"] is prerelease and release["tag_name"] == tag, "release readback is not the requested draft")
        actual = release["assets"];require(len(actual) == len(names) and {a["name"] for a in actual} == set(names), "draft asset set differs")
        for asset in actual:
            row = snapshots["downloads/" + asset["name"]]
            require(asset["state"] == "uploaded" and asset["size"] == row["bytes"] and asset["digest"] == "sha256:" + row["sha256"], "draft asset readback differs")
        plan["draftCreated"] = True;plan["draftUrl"] = release["html_url"]
        (output / "draft-created.tmp").write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
        (output / "draft-created.tmp").rename(output / "draft-created.json")
    return plan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--run-id", required=True, type=int)
    parser.add_argument("--notes", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--create-draft", action="store_true")
    parser.add_argument("--checklist-reviewed", action="store_true")
    args = parser.parse_args()
    try:
        require(tomllib is not None, "draft preparation requires Python 3.11 or newer")
        plan = prepare(Path.cwd(), GitHub(), args.tag, args.run_id, args.notes, args.out, args.create_draft, args.checklist_reviewed)
        print("Draft created: " + plan["draftUrl"] if plan["draftCreated"] else "Preflight complete; no GitHub Release created.")
    except HTTPError as error:
        parser.exit(1, f"draft preparation refused: GitHub HTTP {error.code}\n")
    except (ValueError, OSError, KeyError, TypeError, RecursionError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        parser.exit(1, f"draft preparation refused: {error}\n")


if __name__ == "__main__":
    main()
