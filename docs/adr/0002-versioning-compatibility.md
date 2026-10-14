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
