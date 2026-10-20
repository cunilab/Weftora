---
id: WFT-EVID-011
title: Phase 6 authoring CLI evidence
status: proposed
---

# Phase 6 authoring CLI evidence

## Starter workflow (P6-01)

`weftora new <story-dir>` creates a standalone minimal Story from templates embedded in the CLI. It needs no repository or example Story at runtime. The [getting-started guide](../getting-started.md) covers the author workflow.

The default Engine CLI suite includes six scaffold tests on each host: generated Story validation, both headless branches and export plumbing; refusal of existing files and empty/populated directories; missing-parent refusal; usage errors; two concurrent creators; and Unix symlink or Windows junction refusal. Existing contents are preserved. The export integration fixture uses a fake player only for copying; the separate proof below uses the actual player.

`scripts/verify-new-story.py` creates a fresh Story with spaces and Unicode in its path, checks it, completes both branches, verifies repeat creation refuses without changes, exports with a frozen actual player and runs exported preactivation validation. It compares source inventories and tool hashes, and retains the matching CLI outside the exported game. Its assertions remain enabled with Python `-O`. Windows CI runs the proof and uploads `weftora-windows-new-preview-<commit>` for 14 days.

### macOS validation

Source: `1b1b5dc9428e47cb9b006c1cb02a96e53edd024c`, MacBookAir development host. The [machine-readable starter report](./phase-6-new-story-macos.json) records generated-file hashes, both branch results and the frozen CLI/player hashes. Tools were built before this proof; it performs no Rust build. This is debug-tool evidence.

| Check | Result |
| --- | --- |
| `cargo build --workspace --locked` | Passed |
| `cargo test-all -- --test-threads=1` | 382 passed, zero failed/ignored |
| Scaffold integration suite | Six passed on macOS; Windows live/dangling junction case awaits CI |
| Default and authored-Story CLI/renderer inventories | All six scaffold tests in both; the 14 authored-Story tests only in the feature-enabled inventory |
| `python3 -B -O scripts/verify-new-story.py --cli target/debug/weftora --player target/debug/weftora-player --out <fresh-path>` | Both branches completed; existing project refused; actual exported player validated; source/tree and tool hashes preserved |
| Clippy, default and Story features, all targets | Passed with warnings denied |
| Workspace fmt and shared matrix rustfmt | Passed |
| Python helper tests | Six passed |
| `cargo xtask deps`, `cargo deny --all-features check`, `cargo xtask docs` | Passed |

Claude Opus 5.5 medium reviewed the source/tests/proof with tools disabled. The proof now includes directories in its preservation inventory and keeps the testing CLI outside the exported game; the Windows test exercises a dangling junction as well. The repository's existing `* text=auto eol=lf` pins embedded template line endings. No new dependency was added.

[Source CI run](https://github.com/cunilab/Weftora/actions/runs/39370124773) was still in progress when this record was written. Required Windows CI results and native observations are separate from the macOS results above.

## Remaining acceptance

Headless execution and exported `--validate` do not establish a rendered window, native input, clean-machine operation, release-profile behavior or mismatched-tool enforcement. Windows native starter preview is queued in [issue #5](https://github.com/cunilab/Weftora/issues/5). P6-02 onward and the Phase 6 release gate remain open; see the [success criteria](../success-criteria.md#phase-6--windows-foundation-release).
