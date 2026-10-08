---
id: WFT-ROADMAP-001
title: Weftora Roadmap
type: roadmap
doc_version: 0.5.1
status: proposed
implementation: not_started
created: 2026-09-29
updated: 2026-10-08
last_reviewed: null
owner: Weftora Maintainers
scope: windows-first
product_release: unreleased
related_pr: 1
supersedes: null
---

# Roadmap — Rust-Native Weftora

**Goal:** Weftora — standalone creator-friendly Ren'Py/Yarn alternative. Prebuilt player runs external Story packages; Rust crates internal, not creator framework. **Windows x64 only until Phase 6 exit gate; other OS ports later.** No feature marked done until its acceptance gate passes.

**Scope lock:** Phases 0–6 target native Windows x64 (`x86_64-pc-windows-msvc`), including CLI, player, sample games, save/load, packaging and regression. Preserve generic cross-platform interfaces now; **no macOS/Linux/Android/iOS/Web builds, debugging, CI or release work** before Windows foundation succeeds. See [platform plan](./docs/platforms.md).

**Completion policy:** every phase must pass **all** required test IDs in [Success Criteria](./docs/success-criteria.md). Each PASS includes CI/manual repro evidence, source commit, toolchain/Windows details and expected vs actual result. `[x]` alone ≠ success. Phase 7 remains locked until all Windows Phase 6 criteria pass.

## Phase 0 — Contract + Rust workspace

- [ ] Fix first target: Windows x64 (`x86_64-pc-windows-msvc`); pick minimum supported Windows version through testing; choose project license/dependency policy.
- [ ] Create Cargo workspace; pin tested Rust toolchain and `Cargo.lock` for Windows x64 development.
- [ ] Create `weftora-api`, `weftora-core`, `weftora-story`, CLI/player stubs; design dependency firewall.
- [ ] Specify standalone Windows `weftora.exe` CLI + `weftora-player.exe` release artifacts; Stories remain non-Cargo source folders.
- [ ] Define manifest v1, schema/API compatibility, stable IDs and capability checks.
- [ ] Create `stories/sample-vn/` manifest and invalid fixtures; write ADRs.
- [ ] CI: fmt, clippy, tests, headless forbidden dependencies/unsafe assertions.

**Success:** P0-01..P0-05 — locked Windows build, enforced dependency firewall, versioned manifest positive/negative tests, documented targets/licenses, green CI. [Evidence/tests](./docs/success-criteria.md#phase-0--contract--rust-workspace).

## Phase 1 — Headless engine kernel

- [ ] Boot/load/play/shutdown lifecycle and module/service registry.
- [ ] Typed state store, defaults/scopes, notifications, stable IDs.
- [ ] Typed command registry, errors, cancellation and async sequencing.
- [ ] Signal bus ordering, cleanup and recursion protection.
- [ ] Content catalog loader; missing/duplicate ID diagnostics.
- [ ] CLI skeleton: `new`, `check`, `run` (headless initially), basic logging/trace; no Rust required for Story author.
- [ ] Headless golden tests for deterministic sequence/state.

**Success:** P1-01..P1-06 — lifecycle cleanup, typed state, command cancellation, signal ordering/loop guards, Story catalog loading, 10/10 deterministic headless runs. Full Story event scheduler belongs Phase 4. [Evidence/tests](./docs/success-criteria.md#phase-1--headless-kernel).

## Phase 2 — Bevy + Yarn feasibility spikes

- [ ] Pin/test Bevy renderer on Windows x64 (actual Windows GPU drivers); basic scene, sprite, UI, audio and asset loading.
- [ ] Test `yarnspinner` Rust compiler/runtime without Bevy for dialogue/choices.
- [ ] Test Bevy Yarn integration or bridge: node start/choices, custom commands, mapped variables, error handling.
- [ ] Investigate Yarn Rust WIP gaps; isolate adapter dependency, document fallback.
- [ ] Pick minimal UI primitives and Story-backed asset resolver design.
- [ ] Record findings; lock compatible versions **only after tests**.

**Success:** P2-01..P2-06 — actual Windows renderer/input/audio, Yarn branches + errors, canonical state bridge, adapter isolation, pinned compatibility; unsupported Yarn port must be replaced with adapter satisfying same tests. [Evidence/tests](./docs/success-criteria.md#phase-2--windows-bevy--yarn-adapter-feasibility).

## Phase 3 — First complete playable VN

- [ ] Engine image/view layers, transitions, Windows keyboard/mouse input and audio channels through Bevy.
- [ ] Declarative Story screens: menu, dialogue, choices, buttons, bindings, theme.
- [ ] External Story image/audio catalogs with stable IDs.
- [ ] Single source of truth for Yarn↔Engine variable state.
- [ ] Save container, safe checkpoints, version checks, corruption handling; use Windows user-writable save directory, not packaged Story directory.
- [ ] Sample VN with one branching interaction, character/CG, custom menu and persisted choice.
- [ ] Prebuilt player loads external Story A path; edit dialogue/screens/assets and run without recompiling player.

**Success:** P3-01..P3-06 — playable branching VN, prebuilt Windows player, unchanged SHA-256 after Story edits, UI/input/audio, Yarn state bridge, save/relaunch + corrupted-save handling. [Evidence/tests](./docs/success-criteria.md#phase-3--first-playable-vn--safe-persistence).

## Phase 4 — Story flow + configurable gameplay

- [ ] Story-authored condition/event/action schemas with priority and bounded scheduling.
- [ ] Custom Story rule/command registry using generic host APIs.
- [ ] Reusable Story modules as authored content/config; no built-in relationship/time/quest code in core.
- [ ] Story-defined themed UI and navigation.
- [ ] Prototype Rhai only if declarative rules insufficient; evaluate host API, constraints, hostile scripts and profiling.
- [ ] Document adoption/rejection of optional interpreter with ADR.

**Success:** P4-01..P4-05 — every condition op tested, ordered/cancellable flow, two Story-only gameplay actions, custom screens and validated scripting ADR. Player SHA unchanged. [Evidence/tests](./docs/success-criteria.md#phase-4--story-defined-gameplay-rules--configurable-ui).

## Phase 5 — Second Story proves reuse

- [ ] Sample life-sim-like Story B with locations, schedules, arbitrary stats, events and custom screens.
- [ ] Run Story A and B using **identical compiled player binary**; two standalone external packages, no game-specific Rust compilation.
- [ ] Verify Engine compiles after deleting either Story.
- [ ] Invalid package, missing ID, incompatible schema/save version and command misuse negative tests.
- [ ] Test startup failure rollback and save migration/refusal behavior.

**Success:** P5-01..P5-05 — independently authored life-sim Story B, binary SHA match across A/B, Engine build without Stories, invalid package/save negative tests, no partial state corruption. [Evidence/tests](./docs/success-criteria.md#phase-5--second-story-demonstrates-reuse).

## Phase 6 — Complete Windows foundation + release

- [ ] Windows `weftora.exe` CLI scaffold, validate, run/preview, export; validate portable standalone Windows player + external Story package distribution.
- [ ] Fast Story edit-preview (restart/reload) and source-located validation errors.
- [ ] State, commands, signals, event traces, Yarn node inspect/debug UI.
- [ ] Localization/content pipeline and package version/compatibility checks.
- [ ] Windows x64 release build, asset packaging, dependency security/license checks, GPU/audio profiling.
- [ ] API/how-to docs, sample games, regression suite and first release tag.
- [ ] GUI editor only when core authoring UX and runtime are stable.

**Windows foundation exit gate (ALL required):**

- [ ] Windows x64 release player works on clean Windows machine without Rust/Cargo, dev libs or source checkout.
- [ ] CLI `new`, `check`, `run`, `export` work; export outputs launchable standalone Windows game with Story and assets.
- [ ] VN Story A and life-sim Story B run against exact same compiled Engine binary; content/UI/rules changes do not rebuild it.
- [ ] Yarn dialogue/choices, images/CG, input, UI, audio and Story-defined flow all work in real player.
- [ ] Save/restart/load, safe checkpoints, bad/corrupt saves and incompatible Story versions tested.
- [ ] Invalid manifest/IDs/Yarn/assets fail with useful diagnostics; tests + Windows smoke/regressions pass.
- [ ] External creator can scaffold, preview and export game without editing Engine/Rust or installing Cargo.

**Success:** P6-01..P6-09 — all Windows CLI/export/player, clean-machine creator UX, A/B same binary, save/validation, regression CI, docs/licensing/release artifacts pass. **Tag release only after full PASS**; any failure blocks Phase 7. [Evidence/tests](./docs/success-criteria.md#phase-6--windows-foundation-release-hard-blocker).

## Phase 7 — Multi-platform expansion (BLOCKED until Windows exit)

**Not part of foundation implementation.** Plan interfaces early; start ports only after Phase 6 exits. Track targets/risks in [docs/platforms.md](./docs/platforms.md).

- [ ] **Linux x64:** native player, packaging, X11/Wayland/render/audio/input/storage tests.
- [ ] **macOS ARM64, then Intel if justified:** player `.app`, signing/notarization, lifecycle, fullscreen, packaging and filesystem tests.
- [ ] **Android ARM64:** Activity integration, touchscreen, safe areas, lifecycle, asset packaging, scoped storage, AAB/APK and current Play requirements.
- [ ] **iOS ARM64:** Xcode/signing, app lifecycle, iPhone/iPad UI, storage, embedded Story content, App Store review constraints.
- [ ] **WebAssembly:** browser-compatible loader/assets, WebGL2/WebGPU decision, JS glue, audio gesture unlock, persistence and memory limits.
- [ ] Optional later: Steam Deck/Linux handheld validation, Windows ARM64, Android TV; consoles require restricted SDKs/platform approval.

**Per-target success:** P7-01..P7-07 — **after** Windows foundation PASS, one selected target runs both Stories and existing golden tests, real-device I/O/save, validated export, Windows regression preserved. Each OS marked supported separately. [Evidence/tests](./docs/success-criteria.md#phase-7--per-platform-port-locked-until-p6-pass).

## Guardrails

- **Windows x64 until foundation complete;** no premature other-platform port/CI/export promises. Cross-platform interfaces remain generic.
- **Engine product, not Rust/Bevy framework**; no mandatory per-game Rust project. Internal Rust-native foundation, Bevy candidate backend, Yarn dialogue adapter.
- **Headless core** must not depend on Bevy/Yarn/Rhai or Story packages.
- **No native hot-loaded Rust code** in ordinary Story packages.
- **No genre-specific systems in Engine**; use generic state/actions/views.
- **No unsupported rollback/sandbox guarantees**; prove safety and continuation first.
- **Do not spend first milestones building GUI editor/custom story language**; prove runtime + creator loop.

See [docs/architecture.md](./docs/architecture.md) for boundaries, runtime proposal and research gates.

## Change History

| Date | Version | Change | Reference |
| --- | --- | --- | --- |
| 2026-10-08 | 0.5.1 | Standardize metadata/header and doc lifecycle. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
