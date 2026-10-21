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

[Source CI](https://github.com/cunilab/Weftora/actions/runs/39398030840) subsequently passed all jobs, including required Windows Rust matrices, the upgraded starter/A/B packaged proofs and Engine independence. The current compatibility table/automated fixture checklist is complete. Release-profile reruns (P6-03), native integration (P6-04) and historical-binary guarantees are separate; none is inferred from modeled version metadata.

## Folder packaging and release workflow (P6-05 decision)

[ADR 0013](../adr/0013-folder-release-packages.md) records folders as the runtime container. No Engine archive reader is introduced. ZIP transport for GitHub assets has explicit path/link/bounds and extracted-inventory requirements; its [packager and rejection tests](#zip-transport-validation-p6-05-automated-slice) are implemented below. The P6-05 decision checkbox records this format choice, not a working transport packager or a release acceptance result.

The [release guide](../releasing.md) follows the maintainer's chosen trunk-based flow: reviewed changes into `main`, an exact verified commit, annotated version tag, draft GitHub Release and approved promotion of tested bytes. Current CI has no tag-driven release pipeline. This documentation-only change does not build, tag or publish a release and does not rerun the historical Rust suite. Header/link and diff checks validate the docs; P6-03/release automation and native acceptance remain open.

## Release-profile automation (P6-03 partial)

The `Windows release verification` CI job now runs the full fixture suite with `--release --target x86_64-pc-windows-msvc`, then builds a matching CLI/player pair and runs the maintained starter, Story A edit, screen edit and A/B same-player proofs. Engine independence runs last with `--profile release --target x86_64-pc-windows-msvc --reuse-target`; all Cargo operations and the retained standalone player use that profile/target. Staging remains outside Cargo `target/`. Logs, JSON reports and exported examples are retained in `weftora-windows-release-evidence-<SHA>` for 14 days. The stripped source/target trees are excluded from upload.

Local MacBookAir validation: eight Python helper tests pass, including Cargo-option placement before test-harness arguments and release/default/explicit-target player paths. Documentation headers/links and diff checks pass. [Windows CI 39428560262](https://github.com/cunilab/Weftora/actions/runs/39428560262) subsequently passed all four jobs, including the complete release-profile job and Engine independence. No local release build or native Windows acceptance is claimed. Artifact names use the tested PR merge SHA, as supplied by `github.sha`.

P6-03 stays open: earlier hash/fixture recipes beyond the maintained scripts still need release-specific evidence. The ZIP transport slice is tracked below; clean-machine/native acceptance and tag/draft publishing automation remain pending. This job has read-only repository permissions and creates no tag or GitHub Release.


## ZIP transport validation (P6-05 automated slice)

`scripts/package-folder.py` freezes a reviewed folder inventory, writes a single-root ZIP, extracts into a fresh verification directory and compares names, sizes, SHA-256 hashes, empty directories and executable bits. It rechecks the source before producing the archive checksum. Limits are 10,000 entries and 1 GiB uncompressed data per package. Refusals cover unsafe/Windows-reserved paths, case/Unicode aliases, links/junctions/reparse points, special files, duplicate/missing/extra members, wrong types/sizes/hashes and exceeded limits. Existing output is never overwritten; partial failed output is retained without success/checksum publication. The release operator must control quiescent source/output directories; the helper does not expose arbitrary archive extraction.

The [MacBookAir report](./phase-6-transport-macos.json) records two actual debug-tool exports (Story A and Story B), packaging under Python `-O`, successful inventory/hash round trips and `--validate` using each extracted player. Twenty Python tests run locally: nineteen pass and the native Windows junction test is skipped. Actionlint 1.7.7, docs headers/links and diff checks pass. Sol independently reviewed the transport boundaries; missing reserved console-device names and a pre-extension-space alias were fixed and added to refusal tests.

Windows release CI now packages fresh A/B exports, runs the refusal suite including live/dangling native junctions and validates each extracted game before Engine cleanup. [Windows CI 39544739947](https://github.com/cunilab/Weftora/actions/runs/39544739947) subsequently passed all four jobs, including the ZIP step and native junction refusal test. ZIPs, source inventories, proof reports and `SHA256SUMS` are retained beside the other release evidence; verification trees are excluded to avoid duplicate binaries. This establishes transport plumbing, not an approved authoring-tools package, licence audit, clean-machine/native acceptance, or a published release. See [the operator recipe](../releasing.md#verify-zip-transport).

## Authoring-tools candidate and notice inventory

`scripts/stage-authoring-tools.py` verifies/copies a matching CLI/player pair and adds an author README, workspace MIT/Apache texts and a recursive dependency notice review inventory. It collects available licence/copyright/notice files, including nested font notices, with hashes. Missing texts/metadata are explicit and `auditApproved` is always false. Cargo metadata follows normal/build dependencies of both apps under the tool target and excludes dev-only edges; unified workspace features/build tools can overapproximate binary contents. It cannot replace embedded-asset, native-library or redistribution-obligation review.

The [MacBookAir proof](./phase-6-authoring-tools-macos.json) stages existing debug tools, packages/extracts them with verified hashes and runs the full starter proof using the extracted pair: create/check, both headless branches, export and exported-player validation. It inventories 457 packages with 47 missing-text/metadata cases. These are Mac candidate counts, not Windows release audit results. Twenty-seven Python tests run: twenty-six pass and the Windows junction test is skipped locally. Actionlint 1.7.7, cargo-deny under existing exceptions, docs and diff checks pass. No Rust runtime changes or full Rust-suite rerun were needed for this tooling change.

Luna gathered licence evidence. Sol independently reviewed copying and tool identities; canonical path traversal and duplicate/case-colliding notice destinations were fixed with regression fixtures. Windows release CI now stages/packages the toolkit and reruns the starter proof from its extracted binaries before Engine cleanup; this new stage awaits its first run. The toolkit ZIP and dependency inventory are retained as review artifacts, not release assets. Licence/advisory approval, native/clean-machine acceptance and publication remain open; see [the staging recipe](../releasing.md#stage-the-authoring-tools-candidate).

## Pinned upstream notice texts

The [upstream collection](../../third-party/notices/README.md) contains 52 exact package/version mappings to 45 source files at full published Cargo VCS commits, with source URLs and SHA-256 hashes. The toolkit stager supplements packaged crate texts with these offline files only after matching source, SPDX expression and clean published revision. A hash mismatch or changed provenance refuses staging. Nested third-party notices and licensing-policy documents remain distinct from the crate's own licence text.

The [local report](./phase-6-upstream-notices-macos.json) records the current Windows-target metadata inventory (445 packages, 859 collected files, one missing package text: `constgebra 0.1.4`) and Mac development inventory (457 packages, 867 collected files, 17 unresolved text/policy cases). The Windows inventory is a local metadata check, not execution of Windows binaries. The Mac toolkit is restaged, ZIP/hash verified and used after extraction for the full starter proof under Python `-O`. Thirty-one Python tests run: thirty pass and the native Windows junction case is skipped locally. Source fallback tests cover changed hashes, versions/metadata, dirty/replaced revisions, escaped paths and policy-only documents; a corpus test checks every checked-in source file. Sol verified all mappings/hashes and independently fetched sample files; no legal completeness is inferred.

`constgebra` and `dispatch` have no licence text in their pinned repository trees. Newer Apple-binding repositories provide policy-only documents, retained without substituting attribution from another revision. Their missing-text flags remain open, and `auditApproved` stays false. Embedded assets/native libraries, redistribution obligations and advisory exceptions still require audit. The existing Windows toolkit CI run is tracked separately; the new offline fallback behavior awaits Windows CI. No runtime Rust change, tag or release is introduced.

## Developer state inspector (P6-10 partial)

[Developer mode](../debug-tools.md) is enabled only by `weftora run <story> --debug-tools` or the explicit player flag. F12 opens a full-window, paged inspector for declared canonical keys/scopes, including removed declarations. A 16 KiB JSON editor applies existing-key values through the ordinary rule bridge after candidate type/value, Yarn, display and audio validation. Invalid inputs remain inspector errors without touching the live runner. Actual rule/script failures retain existing terminal runtime behavior. Persistent changes invalidate checkpoint capture; session/settings edits retain eligibility unless rules change persistent state. Lines/choices remain snapshots until normal progression.

Story input, save/load shortcuts and queued pointer intents are discarded while open and on opening/closing frames. Pending presentation actions and media still process. The overlay restores prior focus when its entity survives. The regular renderer entry point and ordinary player launch keep the tools disabled; the explicit mode also works in release builds. Command execution, trace and node selection remain open, with no new Story capability, schema or save wire version.

The [MacBookAir report](./phase-6-debug-tools-macos.json) records 396 passing Rust tests, including nine new inspector tests. Both Clippy configurations deny warnings; formatting/shared rustfmt, dependency direction, docs and diff checks pass. Python tooling runs 31 tests: 30 pass and the native Windows junction case is skipped locally. The CLI preview test now checks exact flag forwarding and artifact cleanup in normal and developer modes. Tests cover actual overlay entity construction, capture/default behavior, rejected-edit preservation, rule settlement, save-file preservation, load replacement, removed declarations, AltGr/UTF-8 input and size limits.

Sol independently reviewed the integration and replay/input boundaries. A native Mac launch exposed a duplicate picking component in the overlay label bundle; it was fixed and an actual overlay-spawn regression test added. A subsequent window-creation smoke run survived two seconds after creation without a panic and was terminated by the test. Visual/input acceptance could not be completed because Computer Use Accessibility/Screen Recording permissions were unavailable. This does not substitute for native Windows testing. The full Windows inspector checklist stays in [issue #5](https://github.com/cunilab/Weftora/issues/5); P6-10 remains open.

[The preceding notice-collection CI](https://github.com/cunilab/Weftora/actions/runs/39548346627) passed all four jobs, including Windows release verification. [Inspector CI](https://github.com/cunilab/Weftora/actions/runs/39552475487) subsequently passed all four jobs, including Windows release verification; no tag or release is created.

## Remaining acceptance

Headless execution and exported `--validate` do not establish a rendered window, native input, clean-machine operation, release-profile behavior or executable authenticity. Windows native starter preview is queued in [issue #5](https://github.com/cunilab/Weftora/issues/5). Clean-machine export, release-profile reruns, native integration and the remaining Phase 6 release checks remain open; see the [success criteria](../success-criteria.md#phase-6--windows-foundation-release).


## Developer command runner (P6-10 partial)

The opt-in developer panel now switches between state and command modes with Tab. Command mode accepts the existing strict `command.call` UI action JSON, validates against the live Story rules registry, then dispatches through the ordinary rules/script bridge. Non-command actions, unknown commands and malformed/incorrect arguments are nonfatal input errors. Commands are never dry-run. Actual rule/script failures retain the runtime's terminal-error and partial-write behavior. Persistent effects invalidate checkpoint capture; displayed dialogue/choices remain snapshots. The bounded command buffer survives mode switches and panel reopening; each new Enter press executes again, while a held Enter and a same-frame Tab/Enter do not resubmit.

[The command-runner report](./phase-6-debug-command-macos.json) records 399 passing Rust tests, including twelve developer-tool tests. Both Clippy configurations deny warnings; formatting/shared rustfmt, dependency direction, docs and diff checks pass. Three new tests cover strict rejection and state/checkpoint preservation, once-only execution with derived rules and save-file preservation, and pending/error guards plus partial-write failure semantics. Existing developer-panel tests continue to cover input capture, UTF-8/AltGr, disabled mode, overlay startup and state editing. Claude Opus 5.5 medium challenged the design and independently reviewed the implementation read-only (PASS with limits); its review is static and does not replace test or native acceptance evidence.

The matching CLI/player pair was rebuilt with `--locked`; a fresh Commands Demo export passed identity verification and its developer-mode player remained alive two seconds after native Mac window creation before the test terminated its own process. This verifies startup only, not visual/input behavior.

Native command-entry and release-build checks are appended to the [Windows checklist](../debug-tools.md#windows-acceptance-checklist) tracked by issue #5. Signal/event tracing, Yarn node selection and full P6-10 acceptance remain open. No tag or release is created.


## Developer signal/event trace (P6-10 partial)

The developer panel now cycles state → command → trace → state with Tab. A developer launch enables a runner-local history before the first displayed line; normal launches retain no history. Capture observes the scheduler's existing success/error trace vectors at the command and notification boundaries, preserving signal delivery, matched/skipped event decisions, completed action indexes and partial failure diagnostics without repeating execution. Rule clones never inherit tracing. History keeps 128 rows with a saturating drop count; streaming formatting limits each row to 256 Unicode characters and escapes controls. The read-only view shows twelve unwrapped/clipped rows, scrolls by row/page and clears with Delete. Command input survives the cycle.

[The trace report](./phase-6-debug-trace-macos.json) records 404 passing Rust tests, including sixteen developer-panel tests and one bounded-history model test. Both Clippy configurations deny warnings; formatting/shared rustfmt, dependency direction, docs and diff checks pass. Tests verify strict ordering, skipped conditions, partial failures, bounded formatting/eviction and clone isolation; traced/untraced runs produce identical state, dialogue and save bytes. Trace-only viewing/navigation/clear preserve checkpoint eligibility and save bytes. A successful load candidate starts a fresh history after replay/validation; failed loads preserve the live ring. Capture before initial dialogue commands is tested. Claude Opus 5.5 medium challenged the design and independently reviewed the implementation read-only: PASS with limits.

This observes rules-scheduler dispatch results, not every Engine signal, state write or script step. Unsubscribed state notifications and failures before dispatch have no trace rows. Tail-relative scrolling shifts as new rows arrive; narrow windows clip long rows. Native Windows text/focus/resize/DPI and trace walkthroughs remain in the [developer-tools checklist](../debug-tools.md#windows-acceptance-checklist) tracked by issue #5. Yarn node selection and the full P6-10/release gates remain open. No tag or release is created.

[Command-runner CI](https://github.com/cunilab/Weftora/actions/runs/39554720283) passed all four jobs, including Windows release verification. The trace change requires its own head CI. The matching CLI/player pair was rebuilt with `--locked`; a fresh Commands Demo export passed tool identity verification and its developer-mode player survived two seconds after native Mac window creation before the test terminated its own process. This checks startup only; visual/input acceptance remains unverified.


## Developer Yarn node launch (P6-10 partial)

CLI preview and exported players accept `--debug-tools --start-node <exact-title>` in that order. Node selection is a fresh launch, with Story/Yarn defaults across all scopes, fresh visit/script state and the authored RNG seed; preceding nodes are not replayed. Unknown names fail before CLI child discovery/preview-file creation or renderer window creation. The window identifies the selected node and save/load refusal. The compiled artifact and manifest entrypoint remain unchanged. Existing renderer launch APIs remain available; production and model tests now share player construction.

Both save and load are disabled for the entire selected-node session through the existing store-refusal path. F5/F9 and authored save/load intents cannot access the ordinary slot/backup, even at authored checkpoints. Relaunch without node selection for normal save/load tests. Runtime checkpoint eligibility remains authored (no synthetic checkpoint), and visit counts follow the pinned compiler/runtime's existing increment-on-node-exit behavior. Trace capture begins before the first selected-node command. There is no in-session node picker, jump or alternate saved snapshot.

[The node-launch report](./phase-6-debug-node-macos.json) records 409 passing Rust tests, including nineteen developer-panel tests. Both Clippy configurations deny warnings; formatting/shared rustfmt, dependency direction, docs and diff checks pass. Tests cover exact child argv/artifact lifetime/exit status, invalid flag combinations and pre-window name refusals; selected-node state/first line/once-only actions/trace/visits/checkpoint behavior; and preservation of ordinary slot and backup bytes through direct, keyboard and authored save/load refusals. Claude Opus 5.5 medium challenged the design, including normal-save overwrite risk; the implementation refuses save/load for these launches. Its final read-only implementation review returned PASS with limits; main verified save-status and other production I/O entry points as well as pinned compiler node enumeration.

The four P6-10 tooling behaviors are implemented, with node selection supplied at launch. Full release-build manual acceptance remains open in the [Windows checklist](../debug-tools.md#windows-acceptance-checklist) tracked by issue #5. Node content may depend on earlier state and retains normal Story error behavior. [Trace CI](https://github.com/cunilab/Weftora/actions/runs/39557143507) passed all four jobs, including Windows release verification; this node change needs its own head CI. Matching tools were rebuilt with `--locked`; a fresh Commands Demo export passed tool identity verification. Native Mac startup passed for adjacent Story, explicit Story and explicit artifact launch forms at `Next`, each observed alive for two seconds after window creation before terminating its own process. All three forms rejected a missing node before opening a window. This is startup evidence only; visual/input acceptance remains unverified. No tag or release is created.

## Localisation catalog validation (P6-11 partial)

The optional manifest `localization` block and `localization.catalog/1` capability now load strict, bounded JSONC catalogs. The [format guide](../localization.md) defines language/path/key/text limits and source ownership: Yarn supplies base dialogue, the base catalog supplies UI keys, and every translated catalog must cover exactly those UI keys and all compiled dialogue strings, including options. Canonical ICU identifiers are validated without panic. Dialogue substitution index sets must match; reordering and repetition are allowed. Catalogs accept mixed-case/long explicit Yarn IDs and the pinned compiler's generated decimal CRC32 IDs, while recommending explicit tags for stability. UI values remain literal strings.

The [English/Indonesian/Japanese fixture](../../examples/localization-demo/README.md) validates and completes both source-language branches. The [MacBookAir report](./phase-6-localization-catalog-macos.json) records 417 passing Rust tests, including six CLI and two Story localisation tests. Twenty configuration/catalog mutations refuse `check`, headless `run` and `export` without activation or output creation. Tests also cover strict JSONC/bounds, canonical region/script tags, placeholder reorder/repeat and rejection cases, compiler-free artifact coverage, source/artifact preservation, and a real existing outside-root catalog refused by path policy. Both Clippy configurations deny warnings; formatting, shared vendor rustfmt, dependency direction and documentation/diff checks pass. Final diagnostic escaping changes pass the eight focused tests.

Matching CLI/player tools were rebuilt with `--locked`. A fresh export contains all three catalogs with matching content hashes. Its actual compiler-free player validates the package, specifically refuses a missing Japanese choice translation without changing the artifact, and validates again after repair. This checks exported validation, not a rendered window. Claude Opus 5.5 medium challenged and independently reviewed the supplied implementation read-only. Its blocking Yarn-ID grammar finding and weak traversal test were corrected; follow-up review returned PASS with limits. Main also escaped invalid-key/tag diagnostics and added a control-character regression check.

This is the catalog-validation step only. Live language switching, translated UI bindings, Japanese font integration, markup validation and native screenshots remain unimplemented/unverified. Catalog coverage permits intentionally empty strings and does not establish translation quality. No language setting, save fields or dialogue artifact format change is introduced. Existing issue #5 tracks native localisation acceptance; P6-11 stays unchecked. [Node-preview CI](https://github.com/cunilab/Weftora/actions/runs/39559932560) and the [three-language scope documentation CI](https://github.com/cunilab/Weftora/actions/runs/39581041666) passed all jobs; this implementation requires its own head CI. No tag or release is created.
