#!/usr/bin/env python3
"""Build/test an isolated source copy without Story A/B; never remove the originals."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys

STORIES = ("examples/story-a", "examples/life-sim")
CLI_TESTS = (
    "both_story_a_branches_finish_with_distinct_routes_and_one_attempt",
    "real_story_branch_checkpoints_replay_the_opening_once_and_resave_identically",
    "a_day_combines_clock_schedule_resources_inventory_and_seeded_pay",
    "work_is_conditional_on_time_energy_and_the_permit",
    "every_checkpoint_replays_actions_rng_and_collections_once",
    "story_a_and_b_reject_each_others_saves_without_output_or_file_changes",
    "loaded_days_can_take_an_alternate_branch_and_skip_work",
    "pay_edit_with_schema_bump_refuses_old_save_and_preserves_both_files",
    "story_matrix_rejects_invalid_packages_and_loads_without_publishing_or_replacing_files",
)
RENDER_TESTS = (
    "saves::tests::story_a_every_checkpoint_reconstructs_the_same_settled_view_audio_and_state",
    "saves::tests::life_sim_checkpoints_restore_the_day_without_repeating_costs_or_pay",
    "saves::tests::story_matrix::story_matrix_failed_loads_preserve_live_state_dialogue_ui_audio_and_files",
)


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def inventory(repository):
    repository = repository.resolve()
    result = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
                            cwd=repository, capture_output=True, check=True)
    files = sorted(set(name.decode("utf-8") for name in result.stdout.split(b"\0") if name))
    records = {}
    for name in files:
        path = repository / name
        require(not path.is_symlink(), f"source symlink not supported: {name}")
        require(path.resolve() == path and path.is_file(), f"invalid or aliased source file: {name}")
        records[name] = digest(path)
    return records


def copy_workspace(repository, destination, records):
    destination.mkdir(parents=True, exist_ok=False)
    copied = {}
    for name, expected in records.items():
        if any(name == story or name.startswith(story + "/") for story in STORIES):
            continue
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(repository / name, target)
        require(digest(target) == expected, f"source changed while copying: {name}")
        copied[name] = expected
    require("Cargo.lock" in copied and any(name.startswith("vendor/") for name in copied),
            "source inventory must include Cargo.lock and vendored runtime")
    return copied



def check_control(text, names):
    for name in names:
        require("test " + name + " ... FAILED" in text, f"missing-fixture test silently skipped: {name}")
        marker = "---- " + name + " stdout ----"
        require(marker in text, f"missing failure details: {name}")
        section = text.split(marker, 1)[1].split("\n---- ", 1)[0]
        require("cannot access Story root" in section, f"negative control failed for an unrelated reason: {name}")


def message_has_path(message, path):
    if os.name == "nt":
        def normalize(value):
            return value.replace("\\\\?\\", "").replace("\\", "/").casefold()
        return normalize(path) in normalize(message)
    return path in message


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--reuse-target", action="store_true", help="Reuse repository Cargo cache; no cold-build claim")
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[1]
    output = args.out.resolve()
    require(not output.is_relative_to(repository), "proof output must be outside the checkout")
    output.mkdir(parents=True, exist_ok=False)
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=repository, capture_output=True, text=True, check=True).stdout.strip()
    dirty = bool(subprocess.run(["git", "status", "--porcelain"], cwd=repository, capture_output=True, check=True).stdout)
    records = inventory(repository)
    require(all(any(name.startswith(story + "/") for name in records) for story in STORIES),
            "both original Stories must be present before making the stripped copy")
    workspace = output / "workspace"
    copied = copy_workspace(repository, workspace, records)
    (output / "source-inventory.json").write_text(json.dumps(copied, indent=2) + "\n", encoding="utf-8")
    target = repository / "target" if args.reuse_target else output / "target"
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_TERM_COLOR="never")
    commands = []

    def absent():
        require(all(not (workspace / story).exists() for story in STORIES), "a removed Story reappeared")

    def cargo(name, directory, arguments, expected=0):
        print(f"{name}: running", flush=True)
        result = subprocess.run(["cargo"] + arguments, cwd=directory, env=environment,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=3600)
        (output / (name + ".log")).write_bytes(result.stdout)
        require(result.returncode == expected, f"{name}: exit {result.returncode}; see {name}.log")
        commands.append({"name": name, "command": ["cargo"] + arguments, "exitCode": result.returncode})
        print(f"{name}: exit {result.returncode}", flush=True)
        return result.stdout.decode("utf-8", errors="replace")

    # List the full suite in the untouched checkout; run it separately with cargo test-all.
    listed = cargo("story-test-inventory", repository, ["test-all", "--", "--list"])
    for name in CLI_TESTS + RENDER_TESTS:
        require(name + ": test" in listed.splitlines(), f"mandatory Story test missing: {name}")
    try:
        absent()
        # Shared targets can retain env!(CARGO_MANIFEST_DIR) from another source tree.
        # Reuse dependency artifacts only; workspace packages must rebuild from this copy.
        cargo("workspace-cache-reset", workspace, ["clean", "--workspace", "--profile", "dev", "--locked"])
        executable = "weftora-player.exe" if os.name == "nt" else "weftora-player"
        built_player = target / "debug" / executable
        require(not built_player.is_symlink(), "refusing a symlink Cargo player output")
        if built_player.exists():
            built_player.unlink()
        cargo("stripped-build", workspace, ["build", "--workspace", "--locked"])
        require(built_player.is_file(), "stripped build did not recreate the player output")
        standalone = output / "standalone"
        standalone.mkdir()
        player = standalone / executable
        shutil.copy2(built_player, player)
        player_hash = digest(player)
        absent()
        engine_tests = cargo("stripped-tests", workspace, ["test", "--workspace", "--locked", "--", "--test-threads=1"])
        require(all(name not in engine_tests for name in CLI_TESTS + RENDER_TESTS),
                "Story tests leaked into the default Engine suite")
        absent()
        # Enabling the authored suites must fail on absent content, rather than quietly skip it.
        cli_control = cargo("missing-story-cli-control", workspace,
                            ["test", "--locked", "-p", "weftora-cli", "--features", "story-tests", "--no-fail-fast",
                             "--test", "story_a", "--test", "life_sim", "--test", "story_matrix", "--", "--test-threads=1"], expected=101)
        check_control(cli_control, CLI_TESTS)
        render_control = cargo("missing-story-render-control", workspace,
                               ["test", "--locked", "-p", "weftora-render-bevy", "--features", "story-tests", "--lib",
                                "--", "--test-threads=1"], expected=101)
        check_control(render_control, RENDER_TESTS)
        failures = []
        for name, arguments, missing in [
            ("story-a", [str(workspace / STORIES[0])], str(workspace / STORIES[0])),
            ("story-b", [str(workspace / STORIES[1])], str(workspace / STORIES[1])),
            ("default", [], str(standalone / "story")),
        ]:
            previous = None
            for _ in range(2):
                result = subprocess.run([str(player)] + arguments, cwd=standalone,
                                        capture_output=True, timeout=30)
                message = result.stderr.decode("utf-8", errors="replace")
                require(result.returncode == 1 and not result.stdout, f"missing {name}: expected exit 1 and no stdout")
                require("manifest.jsonc" in message and "cannot access Story root" in message and message_has_path(message, missing),
                        f"missing {name}: wrong diagnostic: {message}")
                require(not any(token in message.lower() for token in ["panicked at", "wgpu", "bevy_render"]),
                        f"missing {name}: unexpected panic/backend initialization")
                require(previous is None or previous == result.stderr, f"missing {name}: nondeterministic diagnostic")
                previous = result.stderr
            (output / ("missing-" + name + ".log")).write_bytes(previous)
            failures.append({"case": name, "exitCode": 1, "stdoutEmpty": True, "deterministic": True})
        absent()
        require(digest(player) == player_hash, "tested standalone player changed")
    finally:
        if args.reuse_target:
            handling_failure = sys.exc_info()[0] is not None
            try:
                cargo("workspace-cache-release", workspace, ["clean", "--workspace", "--profile", "dev", "--locked"])
            except (Exception, SystemExit):
                if not handling_failure:
                    raise
                print("Workspace cache cleanup also failed; preserving the original proof failure.", flush=True)
    require(inventory(repository) == records, "original checkout source changed during proof")
    report = {"check": "P5-03", "platform": platform.platform(), "baseCommit": head, "workingTreeDirty": dirty, "removedStories": list(STORIES),
              "sourceFileCount": len(copied), "sourceInventorySha256": digest(output / "source-inventory.json"),
              "cargoCacheReused": args.reuse_target, "workspaceArtifactsReset": True, "sharedWorkspaceArtifactsReleased": args.reuse_target, "playerSha256": player_hash,
              "mandatoryStoryTestsListed": len(CLI_TESTS) + len(RENDER_TESTS),
              "defaultEngineReportedTestPasses": sum(map(int, re.findall(r"test result: ok\. (\d+) passed", engine_tests))),
              "commands": commands, "missingStoryFailures": failures, "originalSourceUnchanged": True,
              "scope": "Default Engine suite passes without A/B. Authored suites are listed separately and must fail when explicitly enabled without fixtures. Workspace artifacts are reset before build; only dependency cache may be reused. No native rendering acceptance or cold-build claim."}
    (output / "engine-independence-proof.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"P5-03 passed; logs/report/player: {output}", flush=True)


if __name__ == "__main__":
    main()
