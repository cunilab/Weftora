---
id: WFT-EVID-011
title: Phase 6 authoring and export evidence
status: proposed
---

# Phase 6 authoring and export evidence

## Starter workflow (P6-01)

`weftora new <story-dir>` creates a standalone minimal Story from templates embedded in the CLI. It needs no repository or example Story at runtime. The [getting-started guide](../getting-started.md) covers the author workflow.

The default Engine CLI suite includes six scaffold tests on each host: generated Story validation, both headless branches and export plumbing; refusal of existing files and empty/populated directories; missing-parent refusal; usage errors; two concurrent creators; and Unix symlink or Windows junction refusal. Existing contents are preserved. The export integration fixture uses an identity-aware test executable for copying (still not a renderer); the separate proof below uses the actual player.

`scripts/verify-new-story.py` creates a fresh Story with spaces and Unicode in its path, checks it, completes both branches, verifies repeat creation refuses without changes, exports with a frozen actual player and runs exported preactivation validation. It compares source inventories and tool hashes, and retains the matching CLI outside the exported game. Its assertions remain enabled with Python `-O`. Windows CI runs the proof and uploads `weftora-windows-new-preview-<commit>` for 14 days.

### macOS validation

Source: `1b1b5dc9428e47cb9b006c1cb02a96e53edd024c`, MacBookAir development host. The [machine-readable starter report](./phase-6-new-story-macos.json) records generated-file hashes, both branch results and the frozen CLI/player hashes. Tools were built before this proof; it performs no Rust build. This is debug-tool evidence.

| Check | Result |
| --- | --- |
| `cargo build --workspace --locked` | Passed |
| `cargo test-all -- --test-threads=1` | 382 passed, zero failed/ignored |
| Scaffold integration suite | Six passed on macOS and required Windows CI, including the live/dangling junction case |
| Default and authored-Story CLI/renderer inventories | All six scaffold tests in both; the 14 authored-Story tests only in the feature-enabled inventory |
| `python3 -B -O scripts/verify-new-story.py --cli target/debug/weftora --player target/debug/weftora-player --out <fresh-path>` | Both branches completed; existing project refused; actual exported player validated; source/tree and tool hashes preserved |
| Clippy, default and Story features, all targets | Passed with warnings denied |
| Workspace fmt and shared matrix rustfmt | Passed |
| Python helper tests | Six passed |
| `cargo xtask deps`, `cargo deny --all-features check`, `cargo xtask docs` | Passed |

Claude Opus 5.5 medium reviewed the source/tests/proof with tools disabled. The proof now includes directories in its preservation inventory and keeps the testing CLI outside the exported game; the Windows test exercises a dangling junction as well. The repository's existing `* text=auto eol=lf` pins embedded template line endings. No new dependency was added.

[P6-01 CI](https://github.com/cunilab/Weftora/actions/runs/39370423376) subsequently passed all jobs, including required Windows. Native observations remain separate from CI results.

## Export tool identity (P6-02 automated slice)

The [identity protocol](../adr/0002-versioning-compatibility.md#export-tool-identity-protocol-1) refuses mixed build/compiler/format/target/profile/executable-role identities before export creates output. Metadata records both identities and verified CLI/player hashes; existing Story/compiler/content metadata remains. Source fingerprinting needs no Git and excludes authored Stories and project docs. The exported player remains compiler-free. No new package was downloaded: build-time hashing and player JSON reporting reuse already-locked libraries.

Three default Engine integration tests cover machine-readable CLI identity/version output; every mismatched identity field, missing/unknown fields and malformed/oversized responses; failed/hanging/missing/non-executable players. Refusals preserve source bytes, an existing empty output and missing output ancestors. Identity-aware Rust responders are test fixtures, not real player acceptance. The original six scaffold tests and export copying tests still run.

The starter and Story A/B proof scripts now read actual CLI/player identities and compare them to metadata/tool hashes on every export. The A/B proof additionally selects the actual CLI as a player and requires an executable-role mismatch on both Stories, for both absent and existing-empty destinations. Existing Windows CI starter and life-sim artifacts retain the upgraded proof reports and diagnostics.

The [actual changed-source proof](./phase-6-tool-drift-macos.json) retains a real debug player, appends one recorded comment to Engine CLI source and rebuilds only the CLI. Both tools remain release `0.0.1`, but their build IDs differ. A/B exports each refuse for absent and existing-empty destinations: four cases, no stdout, source/output unchanged. The source bytes are then restored and both tools rebuilt; identities return to the baseline. This is a real current-code build mismatch, not a historical-release or downgrade fixture. The report records the controlled append, source hashes and identities so the experiment can be reproduced.

### Paired-tool validation

Source `88c49359411d3d03b53a0ff9b3fd74fc7d0cb6c7`, MacBookAir development host. The [paired-tool report](./phase-6-tool-pair-macos.json) records frozen identities/hashes, commands, the starter branches and four A/B wrong-role refusals. The starter, A, B, pay edit and both compatible revisions export and validate with the same actual player; their metadata matches the queried identities and frozen hashes. No Rust build occurs between these exports.

| Check | Result |
| --- | --- |
| `cargo test-all -- --test-threads=1` | 385 passed, zero failed/ignored |
| Clippy, default and Story features, all targets | Passed with warnings denied |
| Workspace fmt, shared matrix rustfmt | Passed |
| Python helper tests | Six passed |
| `cargo xtask deps`, `cargo deny --all-features check`, `cargo xtask docs` | Passed |
| Player normal/build dependency tree | No authoring compiler |
| Workspace build, optimized-Python starter and A/B proofs | Passed |
| Actual changed-source CLI with retained player | Four A/B fresh/empty-output refusals; original source and baseline identities restored |
| [Source CI](https://github.com/cunilab/Weftora/actions/runs/39382990097) | All jobs passed, including required Windows, upgraded starter/A/B proofs and Engine independence |

Native acceptance remains separate. Sol independently reviewed fingerprinting, identity parsing/timeout, refusal ordering and copied-player hash verification without editing or building; no actionable production defect was found. Claude Opus 5.5 medium supplied a tools-disabled design challenge; host-target scope, build configuration inputs, source filtering and protocol limits are documented explicitly.

## Downgrade compatibility (P6-09 automated slice)

[ADR 0002](../adr/0002-versioning-compatibility.md#downgrade-policy-clarification-phase-6) now distinguishes compatibility downgrades from compatible release rollbacks. The current reader refuses a valid save with a newer `writerEngineApi` by SemVer precedence, ignoring build metadata. A higher saved `saveSchema` has a dedicated refusal with Story-recovery guidance. Story/Engine release numbers alone still do not reject saves. The API guard is deliberately conservative: it does not claim that every newer-API payload would be structurally unreadable. Wire format 1 is unchanged.

The default save suite covers patch/minor/major API differences, prereleases, ignored build metadata, unchanged files/live state and compatible release-number rollback. The existing schema-downgrade test now uses the actual Store load path and checks slot/backup preservation instead of skipping those assertions.

A/B shared fixtures add newer writer API and newer Story schema rows to both the CLI and player-model failure matrices. Saves have valid checksums and real Story checkpoint snapshots; writer versions/contracts are modeled inputs, not historical executables. CLI failure publishes no stdout and preserves the whole save directory. Player-model F9 refusal preserves state identity/content, dialogue revision/screen, overlay, view/audio tokens and files after deliberately advancing the live game. Positive A/B fixtures load higher Story/Engine release metadata with equal API/schema and restore expected mechanics, without changing saves.

The existing same-player proof now constructs checksummed future-API/schema saves and a compatible release-rollback control for both Stories, exercises the current CLI loader and retains logs/save fixtures in Windows life-sim artifacts. Source and whole save-directory hashes must remain unchanged; the positive control must finish with the expected state. Player-model tests establish renderer integration; CLI traces do not claim native player save loading.

### Validation

Source `f2f33ccb202e05dbd1d688caa432bab9e023e3cc`, MacBookAir development host. The [machine-readable report](./phase-6-downgrade-macos.json) records the eight new CLI/player-model refusal rows, four compatible-release matrix controls and six retained actual-CLI save-load cases, together with tool identities/hashes and commands.

| Check | Result |
| --- | --- |
| `cargo test-all -- --test-threads=1 --nocapture` | 387 passed, zero failed/ignored |
| New A/B downgrade rows | Four CLI and four player-model refusals passed |
| A/B compatible release rollback | Two CLI and two player-model controls passed |
| Retained same-player proof under Python `-O` | Four modeled API/schema refusals and two compatible-load controls passed; source/save directories unchanged |
| Both Clippy configurations, fmt/shared rustfmt | Passed with warnings denied |
| Six Python helper tests, dependency boundaries, cargo-deny, docs | Passed |
| Matching workspace tools and starter proof | Passed |

Astra advised the explicit compatibility-domain policy; Sol independently reviewed save checks, SemVer behavior and A/B state/file preservation. Proof review caught a missing checksum domain prefix; the fixture encoder was corrected to hash `weftora-save-format-1` plus a NUL byte and the exact UTF-8 body before accepting results. Specific refusal diagnostics and the successful compatible control prevent corrupt data from counting as downgrade evidence.

[Source CI](https://github.com/cunilab/Weftora/actions/runs/39398030840) has passed required Windows Rust tests/matrices and the upgraded starter/A/B packaged proofs; final Engine-independence/job completion was still pending when recorded. The current compatibility table/automated fixture checklist is complete. Release-profile reruns (P6-03), native integration (P6-04) and historical-binary guarantees are separate; none is inferred from modeled version metadata.

## Remaining acceptance

Headless execution and exported `--validate` do not establish a rendered window, native input, clean-machine operation, release-profile behavior or executable authenticity. Windows native starter preview is queued in [issue #5](https://github.com/cunilab/Weftora/issues/5). Clean-machine export, release-profile reruns, native integration and the remaining Phase 6 release checks remain open; see the [success criteria](../success-criteria.md#phase-6--windows-foundation-release).
