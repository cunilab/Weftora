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

**Ranges.** `requires.engineApi` uses explicit comparator ranges such as `>=0.2.0 <0.3.0`. Pre-release Engine APIs never satisfy a range unless the range names a pre-release explicitly. Phase 1 syntax is one or more whitespace-separated comparators, each beginning with `=`, `>`, `>=`, `<` or `<=` immediately followed by a complete SemVer. Comparators are intersected; commas, implicit caret ranges, wildcards and partial versions are rejected. The existing `semver` crate evaluates matching and [prerelease rules](https://docs.rs/semver/latest/semver/struct.VersionReq.html). The current API is `0.1.0`, independent of Engine release `0.0.1`; supported capabilities begin with `dialogue.yarn/1`. Schema 1 requires `requires.engineApi` and a SemVer `storyVersion`; schema 0 remains supported for Phase 0 Stories using only their original fields.

**No forced 1.0.** The Phase 6 foundation release may be `0.x` with a published compatibility table.

## Compatibility rules

1. **Story activation:** supported `manifestSchema` → `engineApi` in range → every required capability supported → references valid → compiled dialogue format supported → activate atomically. Any failure rejects before gameplay with the names and versions involved.
2. **Authoring vs. shipping:** `check` and `run` compile Yarn from source; `export` packages compiled dialogue and records the CLI, compiler and player build identities. Exporting with a player the CLI was not released with is refused.
3. **Loading a save** requires: matching `storyId`, a supported `saveFormat`, the Story's current `saveSchema` (or a registered migration from the save's schema) and a known or mapped checkpoint ID. `storyVersion` and the writer's Engine version are recorded for diagnostics only.
4. **Story patches:** changing text, translations, images or audio while keeping `saveSchema` and checkpoint IDs keeps old saves loadable. Adding a saved field with a default may keep the same schema.
5. **Breaking Story changes:** removing or renaming saved fields or checkpoints, or changing what a checkpoint means, requires a `saveSchema` bump plus a migration, or a clear refusal. A version bump alone migrates nothing. A refused save is never deleted or overwritten, and a new game is never started silently.
6. **Downgrades** are unsupported by default and rejected with a clear message.

## Compiled dialogue format 1

The CLI uses the authoring-only `weftora-yarn-compiler`; `weftora-yarn` and the player contain only the runtime. Core, compiler and runtime are pinned to `=0.9.0`; the local runtime instruction-budget patch remains in use. Changes to the lowering or bytecode contract require a new artifact format, independently of the Engine API.

The exported artifact is `compiled/dialogue.json` beside `story/`, not author content inside it. It contains `dialogueArtifactFormat: 1`, `compiler: {id: "yarnspinner-rust", version: "0.9.0"}`, `inputHash`, `bodySha256` and `body`. The body holds the Yarn protobuf program as a byte array, a sorted string table and ordered command locations. Protobuf reuses the upstream encoding without enabling Yarn's optional Bevy/serde dependency graph. The wrapper and body reject unknown fields. The program's BTreeMaps, sorted strings and ordered sites make serialization deterministic; the file ends in one newline.

`inputHash` is SHA-256 over the domain bytes `weftora-dialogue-input-v1` followed by NUL, then the source count as little-endian u64, then each manifest-ordered source's UTF-8 path and contents, each prefixed by its byte length as little-endian u64. Paths come from the validated Story manifest using `/`. `bodySha256` covers the exact UTF-8 bytes of the embedded raw JSON body. The compiler writes compact JSON in fixed field order; the player hashes it without normalizing whitespace or materializing a generic JSON value tree. This checksum detects corruption; it is not a signature or proof of a trusted producer.

The player reads at most 16 MiB plus one sentinel byte, probes the format before deserializing today's wrapper, then checks compiler identity, checksum, bytecode structure, source hash, canonical state/defaults, entrypoint and presentation commands before opening Bevy. The same semantic checks run in the CLI. Structure checks enforce 1..1024 nodes, 1..4096 instructions per node, 65536 total instructions, valid opcode/operand types, finite numbers, valid labels and string IDs, declared variables, known functions, 0..64 substitutions and command metadata matching bytecode. This is not a complete VM stack/type verifier; dynamic errors still fail the runner under its existing panic containment and execution budgets. P2-02 remains open. Stack/options and intermediate string growth within the instruction budget are tracked in [issue #8](https://github.com/cunilab/Weftora/issues/8); a size-bounded artifact does not guarantee a memory-bounded VM.

`weftora run` compiles a fresh preview artifact outside the Story into a temporary directory that survives until the child exits. `weftora export` emits export metadata format 1 for the new layout, records artifact identity/hash separately from Story file hashes and verifies the copied sources and semantic checks before completing. The player accepts an explicit `--dialogue` path for previews, otherwise looks for `../compiled/dialogue.json` relative to the Story. Missing or stale artifacts direct authors to the CLI. `--validate` runs this activation validation without a window; it does not claim rendering, audio or full gameplay verification. Release-tool identity matching remains the Phase 6 requirement.

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
| Downgrade to an older Engine or Story | Rejected with guidance | 6 |
| Export with mismatched CLI/compiler/player | Refused | 6 |

## Consequences

- More fields than a single version number, but each one prevents a specific class of broken save or broken package.
- This ADR owns the version fields and compatibility rules; other docs link here. Changes update this ADR and its fixtures in the success criteria in the same PR.
