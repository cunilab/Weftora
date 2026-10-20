---
id: WFT-ROADMAP-001
title: Weftora Roadmap
status: proposed
---

# Weftora Roadmap

**Strategy: walking skeleton first.** Get an ugly but real game on screen as early as possible, through the real pipeline (Story folder → CLI → player → window), then harden it. The riskiest unknowns — the Rust Yarn Spinner port, Bevy text/UI, and export without a Rust toolchain — are tested in Phase 0, not discovered in Phase 3.

**Rules for every phase**

- A phase is done only when its required checks in [success criteria](./docs/success-criteria.md) pass with evidence. Any explicit deferral must be recorded there and tracked by an issue; it is not a test pass. A ticked box alone means nothing.
- Pending maintainer Windows verification is queued in [issue #5](https://github.com/cunilab/Weftora/issues/5) and does not block later implementation, automated CI or development-host checks. Implementation may advance while acceptance remains open; this does not waive any gate.
- Hardening work lands in the phase that introduces the thing it protects. For example, archive security fixtures arrive when an archive format exists.
- Windows x64 is the only release target and the only place gates are signed off. Maintainers may develop on macOS or Linux; see [platforms](./docs/platforms.md).
- Example Stories live in `examples/`; they are content, never Cargo workspace members, and the Engine never depends on them.

## Phase 0 — Spikes and walking skeleton

Goal: prove the stack end to end with the thinnest possible slice. The phase ends with the go/no-go review in [ADR 0001](./docs/adr/0001-rust-native-engine.md).

**Status:** complete with an explicit P0-10 deferral — built in PR #2. Evidence: [Phase 0 evidence](./docs/evidence/phase-0.md); clean Windows export evidence is tracked in [issue #5](https://github.com/cunilab/Weftora/issues/5). [ADR 0001](./docs/adr/0001-rust-native-engine.md#phase-0-go-no-go-decision) records GO for Phase 1.

- [x] Cargo workspace with `weftora-api`, `weftora-core`, `weftora-story`, `weftora-yarn`, `weftora-render-bevy`, `apps/weftora-player`, `apps/weftora-cli`. Pinned `rust-toolchain.toml`, committed `Cargo.lock`.
- [x] CI runs the merge gate ([architecture §10](./docs/architecture.md#10-drift-protection)) on Windows; the player prints its release ID and build hash (`--version`).
- [x] **Yarn spike:** compile and run a Yarn file with one choice and one variable using the Bevy-independent `yarnspinner` crates. Record the version, licence, gaps and the fallback.
- [x] **Bevy spike:** a window showing a background image, a text line, two clickable choices and a sound, on a real Windows GPU.
- [x] Provisional manifest (`manifestSchema: 0`, explicitly unstable) and a directory loader that confines paths to the Story root.
- [x] `examples/hello-story/`: manifest, one Yarn file, one background, one sound.
- [x] `weftora run examples/hello-story` launches the prebuilt player on it; one choice sets a state variable that the next line reads.
- [x] Editing `hello-story`'s Yarn text and re-running shows the change without rebuilding the player.
- [ ] **Export spike (DEFERRED):** copy the built player and `hello-story` into a folder; it runs on a Windows machine with no Rust installed. Clean-machine evidence is deferred from this gate to [issue #5](https://github.com/cunilab/Weftora/issues/5); Phase 6 export checks still apply.
- [x] Choose the Story authoring file format (JSON, JSONC, TOML, RON or YAML) by ADR, using the `hello-story` files.
- [x] Go/no-go review in ADR 0001: GO for Phase 1 with the explicit P0-10 deferral.

## Phase 1 — Headless kernel

Goal: replace spike code with the real, tested core.

**Status:** revised after independent review; all Phase 1 headless checks passed on Windows at `c49cede`. Implementation is merged in [PR #7](https://github.com/cunilab/Weftora/pull/7). Evidence: [Phase 1 evidence](./docs/evidence/phase-1.md).

- [x] Lifecycle (boot → load → ready → play → shutdown) with ordered init and reverse teardown.
- [x] Canonical typed state: scalar values, defaults, scopes, change notifications.
- [x] Command registry: typed arguments, results, errors, async and cancellation.
- [x] Signal bus: deterministic order, unsubscribe, recursion limit.
- [x] Content catalogs with stable IDs; duplicate and missing ID diagnostics.
- [x] Manifest v1, Engine API range and capability check ([ADR 0002](./docs/adr/0002-versioning-compatibility.md)).
- [x] Directory loader hardening: absolute, UNC and drive paths, `..` traversal, Windows reserved names, case-insensitive collisions, junction and symlink escape, file count and size limits.
- [x] `weftora check` and headless `weftora run` share the loader.
- [x] Golden tests: same inputs produce byte-identical state and event traces.

## Phase 2 — Dialogue and presentation

Goal: full Yarn integration and the presentation primitives a visual novel needs.

**Status:** in progress. Compiled dialogue, typed catalogs, declarative UI and presentation media primitives are implemented with automated checks; see [Phase 2 implementation evidence](./docs/evidence/phase-2.md). Native rendering/audio/input/font/performance results remain maintainer-owned under issue #5. Runtime diagnostics/resource limits pass automated Windows CI; issue #8 is resolved by the merged runtime fixes. No Phase 2 gate is claimed.

- [x] Yarn adapter on canonical state (no shadow variables) with the type mapping in [architecture §6](./docs/architecture.md#6-dialogue-yarn-spinner); Yarn commands mapped to Engine primitives (Story-defined commands follow in Phase 4); file/node diagnostics.
- [x] Compiled dialogue artifact with its own format version and compiler identity; the player refuses unknown formats. [Automated P2-03 evidence](./docs/evidence/phase-2.md#verified-revision).
- [ ] Views and layers: background, characters, CG, overlay; position, scale, opacity, fade, move, crossfade.
- [ ] Audio channels: music, effects, ambience, voice; fades, user volume.
- [x] Choose the declarative UI renderer (Bevy UI or alternative) by ADR.
- [ ] Declarative UI v1: screen, panel, text, image, button, list, choice list, overlay; state bindings; keyboard/mouse focus.
- [x] Asset catalogs for images, audio and fonts; native font/rendering checks remain in issue #5.
- [x] Pin Bevy and Yarn versions; record a dependency licence inventory.
- [ ] Measure a Windows performance baseline; confirm or revise the budgets in [PRD R15](./PRD.md#5-requirements) by ADR.

## Phase 3 — First playable visual novel (Story A) and saves

Goal: a short, complete visual novel played through the prebuilt player.

**Status:** implementation complete. [ADR 0007](./docs/adr/0007-safe-checkpoint-saves.md) defines checkpoint saves; bounded storage, runtime checkpoints, headless save/load, candidate player restoration, Story A, its automated content-only edit proof, author tutorial and Windows test package are implemented. Native Windows testing and the independent tutorial walkthrough remain maintainer-owned in issue #5; no Phase 3 gate is claimed.

- [x] Add `examples/story-a/`, preserving Hello Story golden fixtures: menu, two branches, characters, CG, music, transitions, themed choice UI.
- [x] Saves: container format, Story `saveSchema`, stable checkpoint IDs, atomic write, user-writable Windows location, corruption handling.
- [x] Load pipeline: bounded read → compatibility check → isolated candidate → validate → atomically activate; unsupported migrations refuse safely.
- [ ] Keyboard and mouse navigation, window resizing and readable scaled UI.
- [ ] Prove that editing Story A's dialogue, screens and assets does not change the player binary.
- [x] Author tutorial: how to change Story A without touching Rust; independent Windows walkthrough remains in issue #5.
- [ ] Optional: tag a Windows **developer alpha**. This is not the foundation release.

## Phase 4 — Story-defined rules

Goal: games define their own mechanics without Engine changes.

**Status:** development in progress while earlier native acceptance remains queued in issue #5. Scalar conditions, deterministic events and ordered state actions are implemented in `weftora-flow`; JSONC Story files connect them to the shared dialogue runner and checkpoint candidates; typed declarative commands now run from Yarn, events and screen buttons. [Rule semantics and scope](./docs/rules.md) describe the authoring format and command/UI semantics and bounded script integration. No Phase 4 gate is claimed.

- [ ] Declarative events: trigger → conditions → ordered actions, priority and tie-break, bounded scheduling. Scheduler, JSONC files and dialogue-state notification bridge implemented; native evidence follows.
- [ ] Condition operators on typed values: `==`, `!=`, `<`, `<=`, `>`, `>=`, and, or, not. Evaluator and Story file integration implemented; Windows CI evidence follows.
- [ ] List and map state values with deterministic serialisation ([ADR 0003](./docs/adr/0003-story-rules-language.md)). Flat scalar collections, whole-value actions and save replay are implemented with bounded decoding and automated checks; see [ADR 0009](./docs/adr/0009-flat-collection-state.md). Rhai provides bounded local entry operations; read-only Yarn length/membership helpers now provide scalar views ([ADR 0012](./docs/adr/0012-yarn-collection-queries.md)). Native demonstration remains.
- [ ] Rhai script host: state/signal permissions, shared operation/work and depth/size limits, source diagnostics and no file/network/OS API are implemented ([ADR 0010](./docs/adr/0010-bounded-rhai-host.md)). Canonical seeded integer randomness and replay are implemented ([ADR 0011](./docs/adr/0011-canonical-seeded-randomness.md)); script-internal Engine calls and native acceptance remain.
- [ ] Story-defined commands written in declarative actions or Rhai, callable from screens, events and Yarn. Typed declarative commands and the UI notification bridge are implemented with automated replay checks; Bounded Rhai commands now share these paths; native acceptance remains.
- [ ] Themed custom screens and navigation. The authored [screens demo](./examples/screens-demo/README.md), renderer navigation/focus tests, CLI invalid fixtures and same-player UI edit proof cover P4-06 automatically; native Windows verification remains in issue #5.

## Phase 5 — Second Story proves reuse (Story B)

Goal: a meaningfully different game on the same binary.

- [ ] `examples/life-sim/`: at least two locations, a time/schedule rule, a resource and an inventory-like list, a conditional activity and a custom screen. [Market Day](./examples/life-sim/README.md) implements a finite day with scenario/replay checks; native acceptance remains queued in issue #5.
- [ ] Story A and Story B run on the same player binary (same SHA-256). Automated exports, shared-runner completion and a pay-edit proof are recorded in [Phase 5 evidence](./docs/evidence/phase-5.md); native completion remains.
- [x] The Engine workspace builds and tests pass with both Stories deleted. An isolated-source P5-03 proof runs the default Engine suite without A/B; required CI separately runs the authored-Story suite with `cargo test-all`. See [Phase 5 evidence](./docs/evidence/phase-5.md); isolated macOS proof and required Windows CI passed.
- [x] Negative matrix: invalid packages, missing IDs, cross-Story saves, incompatible schemas and versions, script errors. [A/B matrix evidence](./docs/evidence/phase-5.md#ab-failure-matrix-p5-04) records preactivation refusals, failed-load state/media/navigation preservation and intact save files. Native observations stay in issue #5.
- [x] ADR 0002 fixtures introduced through Phase 5 hold against both Stories, including compatible text/asset patches, old-save reconstruction and modeled version boundaries. [P5-05 evidence](./docs/evidence/phase-5.md#compatible-revisions-and-version-fixtures-p5-05) distinguishes current-player proof from modeled API/writer metadata. Native checks stay in issue #5; Phase 6 release rows remain open.

## Phase 6 — Windows foundation release

Goal: a stranger can make and ship a game on Windows.

- [ ] `weftora new | check | run | export` complete with actionable errors and non-zero exit codes.
- [ ] Decide the release package container (folder or archive). If archive: traversal and decompression-bomb protection and fixtures.
- [ ] Export: prebuilt player + validated Story + compiled dialogue + release metadata (player SHA-256, CLI/compiler identity, content hashes).
- [ ] Debug tools: state inspector, command runner, signal and event trace, start any Yarn node.
- [ ] Localisation pipeline.
- [ ] Licence, dependency licence and advisory audit.
- [ ] Meet the approved performance budgets on the reference Windows machine.
- [ ] Clean-machine walkthrough by someone who does not know Rust.
- [ ] Verify ADR 0002 Phase 6 downgrade and mismatched export-tool release fixtures for both Stories.
- [ ] Publish the compatibility table; tag the release. Pre-1.0 versions are fine.

## Phase 7 — Ports, one at a time (locked until Phase 6 passes)

Each port has its own ADR and passes its own checks; see [platforms](./docs/platforms.md). Proposed order: Linux x64, macOS ARM64, then Android, iOS and Web.

## Guardrails

The [PRD non-goals](./PRD.md#4-non-goals-for-the-foundation) and the [architecture dependency rules](./docs/architecture.md#dependency-rules) apply to every phase. A task that would break one needs a reviewed PRD or ADR change first.
