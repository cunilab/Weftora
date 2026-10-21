---
id: WFT-ADR-002
title: ADR 0002 — Versioning and Compatibility
status: proposed
---

# ADR 0002 — Versioning and Compatibility

## Context

A Story package, the player that runs it, the compiled dialogue it ships and the saves it produces all change on different schedules. Tying them to one version number would either break saves on every text fix or hide real incompatibilities. Nothing has shipped yet, so every version below is illustrative.

## Decision (proposed): independent version domains, introduced when needed

| Domain | Field | Format | Introduced | Rule |
| --- | --- | --- | --- | --- |
| Engine release | `engineRelease` + build ID + SHA-256 | SemVer | Phase 0 | Identifies the exact player/CLI build |
| Story-facing Engine API | `engineApi`; manifest `requires.engineApi` | SemVer | Phase 1 | Independent of release number. Pre-1.0: a minor bump may break. From 1.0: additive = minor, fix = patch, break = major |
| Capabilities | manifest `requires.capabilities`, e.g. `dialogue.yarn/1` | `id/major` | Phase 1 | Coarse feature contracts. Start with very few; add one only when a Story could reasonably run without it |
| Manifest structure | `manifestSchema` | integer | Phase 0 (`0` = unstable), `1` in Phase 1 | Bump on incompatible manifest changes |
| Story release | `storyVersion` | SemVer | Phase 1 | Author's own release; **never** used alone to reject a save |
| Save container | `saveFormat` | integer | Phase 3 | Global Engine format, the same for every Story |
| Story save contract | `saveSchema` | integer per Story | Phase 3 | Bumped by the author when saved state or checkpoint meaning changes incompatibly |
| Compiled dialogue | `dialogueArtifactFormat` + compiler ID/version + input hash | integer | Phase 2 | Separate from the Yarn language version |

Documentation and internal crate versions are unrelated to all of the above.

**Ranges.** `requires.engineApi` uses explicit comparator ranges such as `>=0.2.0 <0.3.0`. Pre-release Engine APIs never satisfy a range unless the range names a pre-release explicitly. Phase 1 syntax is one or more whitespace-separated comparators, each beginning with `=`, `>`, `>=`, `<` or `<=` immediately followed by a complete SemVer. Comparators are intersected; commas, implicit caret ranges, wildcards and partial versions are rejected. The existing `semver` crate evaluates matching and [prerelease rules](https://docs.rs/semver/latest/semver/struct.VersionReq.html). The current API is `0.1.0`, independent of Engine release `0.0.1`; supported capabilities include `dialogue.yarn/1` and `assets.catalog/1` for typed image/audio/font catalogs; Stories using these categories must explicitly require `assets.catalog/1`. Schema 1 requires `requires.engineApi` and a SemVer `storyVersion`; schema 0 remains supported for Phase 0 Stories using only their original fields.

**No forced 1.0.** The Phase 6 foundation release may be `0.x` with a published compatibility table.

## Compatibility rules

1. **Story activation:** supported `manifestSchema` → `engineApi` in range → every required capability supported → references valid → compiled dialogue format supported → activate atomically. Any failure rejects before gameplay with the names and versions involved.
2. **Authoring vs. shipping:** `check` and `run` compile Yarn from source; `export` packages compiled dialogue and records the CLI, compiler and player build identities. Exporting with a player the CLI was not released with is refused.
3. **Loading a save** requires: matching `storyId`, a supported `saveFormat`, a writer `writerEngineApi` no newer than the reader's Engine API, the Story's current `saveSchema` (or a registered migration from the save's schema) and a known or mapped checkpoint ID. `storyVersion` and `writerEngineRelease` remain diagnostic; changing either release number alone does not reject a save.
4. **Story patches:** changing text, translations, images or audio while keeping `saveSchema` and checkpoint IDs keeps old saves loadable. Adding a saved field with a default may keep the same schema.
5. **Breaking Story changes:** removing or renaming saved fields or checkpoints, or changing what a checkpoint means, requires a `saveSchema` bump plus a migration, or a clear refusal. A version bump alone migrates nothing. A refused save is never deleted or overwritten, and a new game is never started silently.
6. **Compatibility downgrades** are unsupported by default. A save from a newer Engine API refuses with both APIs and guidance to use that API or newer. A higher saved Story `saveSchema` refuses with guidance to restore the Story revision that wrote it. Compatible Story content rollbacks and Engine patch rollbacks with unchanged contracts remain eligible to load; release ordering is not a compatibility contract.

## Compiled dialogue format 1

The CLI uses the authoring-only `weftora-yarn-compiler`; `weftora-yarn` and the player contain only the runtime. Core, compiler and runtime are pinned to `=0.9.0`; the local runtime instruction-budget patch remains in use. Changes to the lowering or bytecode contract require a new artifact format, independently of the Engine API.

The exported artifact is `compiled/dialogue.json` beside `story/`, not author content inside it. It contains `dialogueArtifactFormat: 1`, `compiler: {id: "yarnspinner-rust", version: "0.9.0"}`, `inputHash`, `bodySha256` and `body`. The body holds the Yarn protobuf program as a byte array, a sorted string table and ordered command locations. Protobuf reuses the upstream encoding without enabling Yarn's optional Bevy/serde dependency graph. The wrapper and body reject unknown fields. The program's BTreeMaps, sorted strings and ordered sites make serialization deterministic; the file ends in one newline.

`inputHash` is SHA-256 over the domain bytes `weftora-dialogue-input-v1` followed by NUL, then the source count as little-endian u64, then each manifest-ordered source's UTF-8 path and contents, each prefixed by its byte length as little-endian u64. Paths come from the validated Story manifest using `/`. `bodySha256` covers the exact UTF-8 bytes of the embedded raw JSON body. The compiler writes compact JSON in fixed field order; the player hashes it without normalizing whitespace or materializing a generic JSON value tree. This checksum detects corruption; it is not a signature or proof of a trusted producer.

The player reads at most 16 MiB plus one sentinel byte, probes the format before deserializing today's wrapper, then checks compiler identity, checksum, bytecode structure, source hash, canonical state/defaults, entrypoint and presentation commands before opening Bevy. The same semantic checks run in the CLI. Structure checks enforce 1..1024 nodes, 1..4096 instructions per node, 65536 total instructions, valid opcode/operand types, finite numbers, valid labels and string IDs, declared variables, known functions, 0..64 substitutions and command metadata matching bytecode. Additional structural bounds are 2048 declarations, 512-byte variable names, 256-byte node/label/string IDs and 16 KiB UTF-8 string operands/defaults/table entries. Internal bookkeeping is restricted to numeric visiting counters for existing nodes.

The runtime fork caps retained stack values at 1024, accumulated options at 256, batched events at 1024 and individual values/expanded text at 16 KiB. Function argument counts must match the known signature and available stack before collection. Each substitution checks its prospective size before replacement. Storage rejects oversized strings before copying and limits writes to 4096 and their old/new string payloads to 32 MiB between blocking line/choice/end events; command events do not reset these counters. The byte cap permits initialization of all 2048 maximum-size defaults and prevents a roughly 128 MiB notification interval. The player drains its notification queue each frame. Headless replay additionally caps the retained whole-run change history at 4096 entries / 32 MiB (including string payloads and key bytes), before dispatching signals or extending the history. Limits fail the runner permanently and carry file/node context where source provenance is available. These fixed caps address [issue #8](https://github.com/cunilab/Weftora/issues/8); they are not a process RSS budget or a complete VM stack/type verifier.

Node-file provenance stays in memory: compiler debug information supplies it during authoring, and verified source headers restore it on artifact loading. Unknown generated node provenance is reported explicitly rather than attributed to an arbitrary file. Format 1 bytes are unchanged. Literal missing-node jumps fail activation; computed jumps retain runtime validation. Diagnostic-producing invalid syntax, missing nodes and unknown commands share CLI check/run errors. Other upstream parser/runtime panics remain contained; library code does not change the global panic hook. After displaying a runtime failure, closing the player returns failure; ordinary window close/Escape returns success.

`weftora run` compiles a fresh preview artifact outside the Story into a temporary directory that survives until the child exits. `weftora export` emits export metadata format 1 for the new layout, records artifact identity/hash separately from Story file hashes and verifies the copied sources and semantic checks before completing. The player accepts an explicit `--dialogue` path for previews, otherwise looks for `../compiled/dialogue.json` relative to the Story. Missing or stale artifacts direct authors to the CLI. `--validate` runs this activation validation without a window; it does not claim rendering, audio or full gameplay verification. Host-target export now enforces the tool identity protocol below; clean-machine and release-profile acceptance remain Phase 6 requirements.

## Export tool identity protocol 1

Both executables expose `--tool-identity` as one JSON document, before Story loading or any renderer/audio setup. The fields are `identityFormat`, `tool`, `engineRelease`, `buildId`, `target`, `profile`, `dialogueArtifactFormat`, `compilerId` and `compilerVersion`. `--version` keeps its existing human-readable output. The player reports its supported compiler/artifact contract; the compiler is statically linked into the CLI, not loaded from another executable.

The shared runtime build script computes `buildId` from sorted relative paths and bytes of Engine crate/app manifests, build scripts, sources, embedded templates and vendored runtime source, plus root workspace/lock/toolchain/Cargo configuration. It includes target, profile, optimization/debug settings, Rust flags, shared-runtime feature values and `rustc -Vv`. Source/template extensions are explicitly enumerated; hidden files, Story examples, external tests and project documentation are excluded. Git is not required, and absolute checkout paths are not hashed. Cargo watches source directories/files and relevant environment values. Existing Git attributes pin source line endings. This is a conservative source/configuration fingerprint, not a claim of hermetic or bit-for-bit reproducible builds.

Export is currently host-target-only. Before creating export output, the CLI asks its selected player for identity (15-second deadline, at most 8 KiB of JSON, no stdin or inherited stderr). Failed, malformed, oversized, timed-out, unknown-field and unequal identities refuse with same-build installation guidance. All fields must match the CLI's expected player identity, including the executable role. Thus a separately built player from changed Engine source refuses even if the release version is unchanged. `check`, headless execution and scaffolding need no player; preview behavior is unchanged.

The CLI hashes the selected player before querying it and verifies the copied executable against that hash before completing export. Export metadata format 1 retains its existing fields and adds `cli.identity` and `player.identity`; player SHA-256 describes the verified copy. Dialogue compiler identity/input hash and Story version/content hashes retain their existing fields. There is no overwrite or mismatch-bypass flag. Install or build both tools together (`cargo build --workspace --locked` for development).

These self-reported values prevent accidental tool mixing. They do not authenticate an executable selected by the user, prove a historical release, check an exported folder at every later launch or replace signed release packaging. Clean Windows VM, native input/rendering, release-profile reruns and downgrade acceptance remain open. [Phase 6 evidence](../evidence/phase-6.md) records automated checks separately.

## Save format 1

[ADR 0007](./0007-safe-checkpoint-saves.md) defines the bounded/checksummed container and checkpoint replay contract. Stable checkpoint IDs are separate from Yarn node names and instruction offsets. Persistent removals are explicit null entries; compatible new fields keep current defaults. Session resets, current Settings are retained, and runtime visiting counters are restored separately. Until migrations exist, any save-schema mismatch refuses safely without modifying the live game or file. Format/storage implementation does not by itself prove player safe-point or view restoration.

## Downgrade policy clarification (Phase 6)

The earlier blanket downgrade rule conflicted with the separate rule that Story and Engine release numbers do not decide save compatibility. The boundary is now explicit: `writerEngineApi` is a conservative forward-read guard, while Story/Engine **release** numbers remain diagnostic. This may reject a newer-API save whose current fields could otherwise be read; an older reader is not permitted to assume compatibility with future API writers. Use the matching or newer Engine, rather than bypassing the guard. No migration or override is introduced.

API ordering uses SemVer precedence, including prerelease ordering, and ignores build metadata. Equal/older writer APIs remain eligible subject to all existing format, Story ID, schema, checkpoint and state validation. Newer writer API and higher Story schema refusals run before candidate state/replay preparation. Save format 1 and its bytes are unchanged; no new fields are needed. Older implementations did not have this conservative API guard, so this is current-reader enforcement, not a claim that every historical binary refuses a future save.

### Current compatibility table

These are current development-build contracts, not a tagged foundation release. Story release and save schema remain author-owned.

| Domain | Current supported value |
| --- | --- |
| Engine release | `0.0.1`; exact source/configuration build identity and executable SHA-256 are separate |
| Engine API / saved writer API guard | `0.1.0`; newer saved API refuses by SemVer precedence |
| Manifest structure | `manifestSchema` `0` (legacy) and `1` |
| Save container | `saveFormat` `1`; Story `saveSchema` must match until migrations exist |
| Compiled dialogue | `dialogueArtifactFormat` `1` |
| Compiler contract | `yarnspinner-rust` `0.9.0` |
| Tool identity / export metadata | `identityFormat` `1` / `exportFormat` `1` with additive CLI/player identities |
| Capabilities | `dialogue.yarn/1`, `dialogue.collections/1`, `assets.catalog/1`, `ui.screens/1`, `presentation.media/1`, `save/1`, `rules.events/1`, `rules.commands/1`, `state.collections/1`, `rules.rhai/1`, `rules.random/1`, [`localization.catalog/1`, `localization.text/1`](../localization.md) |

[Phase 6 evidence](../evidence/phase-6.md#downgrade-compatibility-p6-09-automated-slice) maps modeled writer fixtures and real current-reader behavior to the checks below. Historical-binary guarantees, release-profile reruns and native acceptance are separate; constructed version metadata does not establish an alternate executable's behavior.

## Required fixtures

| Scenario | Expected | Phase |
| --- | --- | --- |
| Engine patch, API unchanged | Story loads | 1 |
| Engine API additive within the Story's range | Story loads unchanged | 1 |
| API outside range, or missing capability | Rejected, naming requirement and versions | 1 |
| Unknown `manifestSchema` | Rejected before activation | 1 |
| Pre-release Engine API not named by the range | Rejected with guidance | 1 |
| Unknown `dialogueArtifactFormat` | Rejected before activation | 2 |
| Story text/asset patch, same `saveSchema` and checkpoints | Old save loads | 3 |
| Removed checkpoint or `saveSchema` bump | Migrates or is refused; original save preserved | 3 |
| Save from Story A loaded in Story B | Rejected | 5 |
| Corrupt save, failed migration, unknown `saveFormat` | Rejected; live state and save file unchanged | 3 |
| Newer writer Engine API → older reader API | Refused with both APIs and newer-reader guidance; save/live state unchanged | 6 |
| Newer Story save schema → older Story schema | Refused with original-Story guidance; save/live state unchanged | 6 |
| Higher Story/Engine release numbers, same API/schema/checkpoints | Compatible release rollback loads; release numbers alone do not reject | 6 |
| Export with mismatched CLI/compiler/player | Refused | 6 |

## Phase 5 fixture scope

[Phase 5 evidence](../evidence/phase-5.md#compatible-revisions-and-version-fixtures-p5-05) maps every fixture introduced through Phase 5 onto both Stories. Engine/API version inputs in policy tests are modeled explicitly; the export proof retains one actual player SHA-256. Rows introduced in Phase 6 have current-code automated evidence in [Phase 6](../evidence/phase-6.md), including downgrade guidance and mismatched export-tool builds; release-profile and native requirements remain open. This does not claim historical-binary compatibility or native rendering acceptance.

## Consequences

- More fields than a single version number, but each one prevents a specific class of broken save or broken package.
- This ADR owns the version fields and compatibility rules; other docs link here. Changes update this ADR and its fixtures in the success criteria in the same PR.
