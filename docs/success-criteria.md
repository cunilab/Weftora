---
id: WFT-ACCEPT-001
title: Weftora Success Criteria
status: proposed
---

# Weftora Success Criteria

Pass/fail checks for each phase of the [roadmap](../ROADMAP.md). Nothing here has been run yet.

## Rules

- A phase **passes** only when every check in it passes with linked, repeatable evidence. Anything failed or untested means the phase is **blocked**.
- Checks from earlier phases must keep passing. A new phase cannot quietly break an old one.
- Dropping a check requires a reviewed change to this document and the roadmap, not an exception.
- **Where checks run:** everything involving the player, rendering, audio, input, export or performance is verified on **Windows x64**. Headless checks may also run on the maintainer's development host; see [platforms](./platforms.md).
- **Evidence for each check:** check ID, commit SHA, toolchain and dependency versions, command or manual steps, OS (and GPU/driver/audio device for player checks), input fixture, expected vs. actual result, and a link to the log or artifact.
- **Binary identity:** "same player" means the same SHA-256 of `weftora-player.exe`. Equal hashes prove the same binary; each Story must also actually run.
- **Negative behaviour:** invalid input gives a documented error (non-zero exit for the CLI, controlled error screen for the player) — never a panic, a silent success or a partly loaded Story.
- Screenshots and videos can support evidence but are never the only proof of behaviour.

## Phase 0 — Spikes and walking skeleton

| ID | Check | Evidence |
| --- | --- | --- |
| P0-01 | Workspace with the Phase 0 crates and apps builds with a pinned toolchain and committed `Cargo.lock`; the player reports its release ID and build hash | `cargo check --workspace --locked` on Windows x64 in CI; `weftora-player --version` output |
| P0-02 | Headless crates have no direct or transitive dependency on `bevy`, `yarnspinner`, `rhai`, adapters, apps or Stories | Automated `cargo metadata` check in CI; a deliberately injected violation makes CI fail |
| P0-03 | CI blocks merges on every item of the merge gate in [architecture §10](./architecture.md#10-drift-protection), including `cargo deny` | Green CI run plus a failing run from an injected error |
| P0-04 | **Yarn spike:** the `yarnspinner` crates compile and run a fixture with two branches and one variable, headless | Fixture, captured line/choice trace for both branches, crate versions and licence, list of gaps and the chosen fallback |
| P0-05 | **Bevy spike:** on Windows, a window shows a background, a text line, two mouse-clickable choices and plays a sound | Manual steps and log with OS, GPU and driver |
| P0-06 | **Walking skeleton:** `weftora run examples/hello-story` launches the prebuilt player; a choice sets a state variable that a later line reads; both branches reachable | Steps and trace for both branches on Windows |
| P0-07 | Editing `hello-story`'s Yarn text and re-running shows the new text with the **same player SHA-256** | Hash before and after, screenshot or log of the new line |
| P0-08 | The loader rejects a Story file reference outside the Story root (absolute path, `..`) with a clear error | Two negative fixtures |
| P0-09 | The Story authoring file format is chosen by ADR | ADR with `hello-story` files in the chosen format |
| P0-10 | **Export spike:** a folder containing only the built player and `hello-story` runs on a Windows machine with no Rust or Cargo installed | Steps, machine description, log |
| P0-11 | ADR 0001 records the Phase 0 go/no-go decision | Updated ADR |

## Phase 1 — Headless kernel

| ID | Check | Evidence |
| --- | --- | --- |
| P1-01 | Lifecycle runs services in documented order and tears down in reverse; a failed init leaves no half-active Story | Ordered trace; injected init failure with cleanup assertions |
| P1-02 | State get/set/add/remove/exists, defaults, scopes and change notifications for bool/int/finite float/string; type mismatch, missing key and NaN/infinity are explicit errors | Unit and property tests |
| P1-03 | Commands accept typed arguments and return results or typed errors; async commands complete or cancel cleanly; unknown commands and bad arguments are rejected | Integration tests |
| P1-04 | Signals deliver in documented order; nested emits are queued; a signal loop stops with an explicit budget error | Golden trace including nested emits and a loop fixture |
| P1-05 | Catalogs reject duplicate and missing IDs, reporting file and location | Negative fixtures |
| P1-06 | Manifest v1, Engine API range and capability checks behave per the Phase 1 rows of [ADR 0002](./adr/0002-versioning-compatibility.md#required-fixtures) | Fixture per row |
| P1-07 | Path policy on Windows rejects UNC and drive paths, reserved names, case-insensitive collisions, junction and symlink escape, and over-limit file count and sizes; no partial activation | Negative Windows fixtures |
| P1-08 | `weftora check` and headless `weftora run` use the same loader; `check` exits non-zero with actionable output on invalid Stories | CLI tests |
| P1-09 | The same input sequence produces byte-identical normalised state and event traces in 10 of 10 fresh runs | Golden fixtures in the repo and the comparison log |

## Phase 2 — Dialogue and presentation

| ID | Check | Evidence |
| --- | --- | --- |
| P2-01 | Yarn reads and writes canonical state: a choice changes state, an Engine primitive (UI binding or view condition) reads it, a later Yarn condition sees it; wrong types give a diagnostic; an `int` beyond ±2^24 read from Yarn is a Story error, not a rounded value | Integration test and precision fixture |
| P2-02 | Invalid Yarn syntax, a missing node and an unknown command each fail with file and node context, without a panic | Three negative fixtures |
| P2-03 | Compiled dialogue carries `dialogueArtifactFormat` and compiler identity; the player rejects an unknown format; the player binary does not include the Yarn compiler | Fixture and dependency tree of `weftora-player` |
| P2-04 | Views: background, two characters, CG and overlay with position, scale, opacity, fade, move and crossfade | Windows scripted demo and log |
| P2-05 | Audio: music, effects, ambience and voice play, fade and respect user volume | Windows manual steps with audio device recorded |
| P2-06 | Declarative UI v1 renders the initial widgets with state bindings and keyboard/mouse focus; invalid widgets, bindings and actions are rejected by `check` | UI tests plus invalid-screen fixtures |
| P2-07 | Adapters remain outside headless crates; Bevy and Yarn versions are pinned with a licence inventory | Dependency check and ADR or notes |
| P2-08 | A Windows performance baseline is recorded, and the PRD R15 budgets are confirmed or revised by ADR | Baseline report and ADR |
| P2-09 | Image, audio and font catalogs resolve stable IDs; missing and duplicate asset IDs are reported with file and location | Fixtures and negative fixtures |
| P2-10 | The declarative UI renderer is chosen by ADR | ADR |

## Phase 3 — First playable visual novel and saves

| ID | Check | Evidence |
| --- | --- | --- |
| P3-01 | Story A runs from an external folder with menu, two branches, characters, CG, music, transitions and themed choice UI | Walkthrough of each branch on Windows |
| P3-02 | Changing Story A's dialogue, screens, theme and an image shows the changes with the **same player SHA-256** | Hashes and before/after evidence for two separate edits |
| P3-03 | Keyboard and mouse navigation work; the window resizes with readable UI; bindings reflect state | Windows UI steps with state assertions |
| P3-04 | Save, quit, relaunch and load restore state and view at a safe point; saving while a command or transition is running is refused or deferred | Round-trip test and in-flight save test |
| P3-05 | Saves are written atomically to the user data directory, never the Story folder; a corrupt save or failed migration reports an error and leaves the live game and the on-disk save unchanged | Negative fixtures and file-system checks |
| P3-06 | The Phase 3 rows of [ADR 0002](./adr/0002-versioning-compatibility.md#required-fixtures) pass: a Story text patch keeps old saves loading; a removed checkpoint or schema bump migrates or refuses safely | Two Story revisions on the same player |
| P3-07 | The author tutorial lets someone change Story A's dialogue, a screen and an image without touching Rust | Walkthrough notes by someone other than the author of the tutorial |

## Phase 4 — Story-defined rules

| ID | Check | Evidence |
| --- | --- | --- |
| P4-01 | Every condition operator (`==`, `!=`, `<`, `<=`, `>`, `>=`, and, or, not) works on valid types and rejects invalid operands | Table-driven tests covering each operator |
| P4-02 | Events run trigger → conditions → ordered actions by priority and declaration order; circular events stop with a budget error | Golden trace with competing priorities and a cycle |
| P4-03 | Two Story-defined commands, one declarative and one Rhai, run on the unchanged player; a failing action stops the sequence without rolling back earlier writes | Same player SHA-256; success and failure traces |
| P4-04 | List and map state values serialise deterministically and survive save/load; they are not exposed to Yarn directly, and a Yarn function returning a scalar view of them works | Unit tests and round-trip test |
| P4-05 | Rhai host contract ([architecture §7](./architecture.md#7-rules-scripting-rhai)) holds: an infinite loop, deep recursion, an oversized string/array/map, `import`, `eval`, `timestamp()` and a call to a non-allowlisted function each produce a Story error with file and line, with no crash or hang | Hostile-script fixtures |
| P4-06 | A Story replaces screen layout, theme and navigation; invalid widgets or bindings are rejected by `check` | UI tests and invalid fixtures |
| P4-07 | `weftora check` reports Rhai syntax errors with file and line; a seeded random sequence continues identically after save and load | Negative fixture and replay test |

## Phase 5 — Second Story proves reuse

| ID | Check | Evidence |
| --- | --- | --- |
| P5-01 | Story B is a life-sim with at least two locations, a time/schedule rule, a resource, an inventory-like list, a conditional activity and a custom screen | Story B fixture and recorded scenario |
| P5-02 | Story A and Story B both run to completion on the **same player SHA-256**, with no Engine rebuild between them | One hash, launch commands and logs for both |
| P5-03 | With both Story folders removed, the Engine workspace builds and its tests pass; the player shows a controlled error for a missing Story | `cargo test --workspace --locked` log and player error capture |
| P5-04 | Negative matrix across both Stories: invalid packages, missing IDs, script errors, incompatible schemas or versions; no partial activation | Fixture matrix with unchanged prior state |
| P5-05 | A Story A save is rejected by Story B; the version rows of ADR 0002 hold for both Stories | Cross-Story and version fixtures |

## Phase 6 — Windows foundation release

| ID | Check | Evidence |
| --- | --- | --- |
| P6-01 | `weftora new`, `check`, `run` and `export` work; `new` scaffolds a valid Story; invalid input gives non-zero exit and actionable output | CLI integration suite |
| P6-02 | `export` produces a standalone game folder that runs on a clean Windows VM without Rust or Cargo; release metadata records player SHA-256, CLI/compiler identity, Story version and content hashes; mismatched tools are refused | Export fixture, metadata inspection, clean-VM run |
| P6-03 | All earlier SHA-256 and fixture checks pass again on the **release** build | Re-run log |
| P6-04 | Windows integration: dialogue, CG, UI, audio, keyboard/mouse, resize/DPI, focus loss and resume — no crash on the tested configurations | Signed-off matrix with OS, GPU and audio versions |
| P6-05 | The release package container is decided; if it is an archive, traversal and decompression-bomb fixtures are rejected | ADR and negative fixtures |
| P6-06 | CI gates and `cargo deny` licence and advisory checks pass on the pinned toolchain | CI logs |
| P6-07 | Someone who does not know Rust follows the published guide to create, edit, preview and export a complete Story on a fresh machine | Written walkthrough with observed issues and fixes |
| P6-08 | Approved performance budgets are met on the reference Windows machine: 5 cold starts, 100 UI interactions, idle memory for A and B, and `weftora check` time | Measurement log |
| P6-09 | The compatibility table (Engine API, capabilities, manifest, save and dialogue formats) is published, and the Phase 6 rows of ADR 0002 pass | Published table and fixtures |
| P6-10 | Debug tools: inspect and edit state, run a command, view the signal and event trace, start any Yarn node | Manual steps on the release build |
| P6-11 | Localisation: a Story ships two languages; switching language changes UI and dialogue text; a missing translation key is reported by `check` | Fixture and screenshots |
| P6-12 | The release is tagged only after every check above passes | Release checklist linking all evidence |

## Phase 7 — Each port (locked until Phase 6 passes)

Run once per platform. Each platform passes separately.

| ID | Check | Evidence |
| --- | --- | --- |
| P7-01 | Phase 6 has passed; one target is chosen with an ADR covering toolchain, renderer, audio, assets, storage, export and signing | Linked release and ADR |
| P7-02 | Story A and B run unchanged on the target; headless golden tests pass | Target device logs |
| P7-03 | Input, scalable UI, audio, graphics, suspend/resume, assets and saves work on real devices | Device test matrix |
| P7-04 | Export, install and any required signing/notarisation/store steps succeed | Clean-install evidence |
| P7-05 | Invalid Story and save handling and path rules hold on the target | Negative fixtures |
| P7-06 | Windows release checks still pass | Windows CI |
| P7-07 | Only this target is marked supported | Docs update |

## Evidence record template

```yaml
check: P3-04
result: PASS        # PASS | FAIL | BLOCKED | NOT_RUN
commit: "<git SHA>"
player_sha256: "<SHA-256 of weftora-player.exe>"
platform: "Windows 11 <build>; <CPU>; <GPU> <driver>"
toolchain: "<rustc>, <bevy>, <yarnspinner>, <rhai>"
fixture: "examples/hello-story @ <commit>"
steps: "<command or manual procedure>"
expected: "Choice B state and scene restored after relaunch"
actual: "<observed>"
evidence: "<CI run / log / artifact link>"
```
