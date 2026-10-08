# Roadmap — Rust-Native Visual Game Engine

**Status:** v0.3 proposal. **Goal:** creator-friendly Ren'Py/Yarn alternative with headless Rust Engine and replaceable Story packages. No feature marked done until its acceptance gate passes.

## Phase 0 — Contract + Rust workspace

- [ ] Choose desktop target platforms and project license; record dependency/license policy.
- [ ] Create Cargo workspace; pin tested Rust toolchain and `Cargo.lock`.
- [ ] Create `vge-api`, `vge-core`, `vge-story`, CLI/player stubs; design dependency firewall.
- [ ] Define manifest v1, schema/API compatibility, stable IDs and capability checks.
- [ ] Create `stories/sample-vn/` manifest and invalid fixtures; write ADRs.
- [ ] CI: fmt, clippy, tests, headless forbidden dependencies/unsafe assertions.

**Gate:** headless core compiles with no Bevy/Yarn/Story deps; valid manifest accepted, malformed/duplicate/incompatible ones rejected.

## Phase 1 — Headless engine kernel

- [ ] Boot/load/play/shutdown lifecycle and module/service registry.
- [ ] Typed state store, defaults/scopes, notifications, stable IDs.
- [ ] Typed command registry, errors, cancellation and async sequencing.
- [ ] Signal bus ordering, cleanup and recursion protection.
- [ ] Content catalog loader; missing/duplicate ID diagnostics.
- [ ] CLI: `new`, `check`, `run` (headless), basic logging/trace.
- [ ] Headless golden tests for deterministic sequence/state.

**Gate:** external Story manifest registers actions and events; replayed headless inputs produce stable outputs with clear errors.

## Phase 2 — Bevy + Yarn feasibility spikes

- [ ] Pin/test Bevy renderer on initial desktop targets; basic scene, sprite, UI, audio and asset loading.
- [ ] Test `yarnspinner` Rust compiler/runtime without Bevy for dialogue/choices.
- [ ] Test Bevy Yarn integration or bridge: node start/choices, custom commands, mapped variables, error handling.
- [ ] Investigate Yarn Rust WIP gaps; isolate adapter dependency, document fallback.
- [ ] Pick minimal UI primitives and Story-backed asset resolver design.
- [ ] Record findings; lock compatible versions **only after tests**.

**Gate:** window displays background/dialogue/choices; Story Yarn command changes typed Engine state. Failing Yarn spike triggers adapter reassessment, not core redesign.

## Phase 3 — First complete playable VN

- [ ] Engine image/view layers, transitions, input and audio channels through Bevy.
- [ ] Declarative Story screens: menu, dialogue, choices, buttons, bindings, theme.
- [ ] External Story image/audio catalogs with stable IDs.
- [ ] Single source of truth for Yarn↔Engine variable state.
- [ ] Save container, safe checkpoints, version checks, corruption handling.
- [ ] Sample VN with one branching interaction, character/CG, custom menu and persisted choice.

**Gate:** Story A playable/restartable; content, UI and dialogue edits need no Engine Rust changes/recompile.

## Phase 4 — Story flow + configurable gameplay

- [ ] Story-authored condition/event/action schemas with priority and bounded scheduling.
- [ ] Custom Story rule/command registry using generic host APIs.
- [ ] Reusable Story modules as authored content/config; no built-in relationship/time/quest code in core.
- [ ] Story-defined themed UI and navigation.
- [ ] Prototype Rhai only if declarative rules insufficient; evaluate host API, constraints, hostile scripts and profiling.
- [ ] Document adoption/rejection of optional interpreter with ADR.

**Gate:** Story defines gameplay logic/events and different screens without editing Engine core. Optional scripts not assumed secure.

## Phase 5 — Second Story proves reuse

- [ ] Sample life-sim-like Story B with locations, schedules, arbitrary stats, events and custom screens.
- [ ] Run Story A and B using **identical compiled player revision**.
- [ ] Verify Engine compiles after deleting either Story.
- [ ] Invalid package, missing ID, incompatible schema/save version and command misuse negative tests.
- [ ] Test startup failure rollback and save migration/refusal behavior.

**Gate:** two meaningfully different games on one Engine binary; zero game-specific core changes.

## Phase 6 — Creator workflow + release

- [ ] `vge-cli` scaffold, validate, build/pack, run/preview and diagnostics.
- [ ] Fast Story edit-preview (restart/reload) and source-located validation errors.
- [ ] State, commands, signals, event traces, Yarn node inspect/debug UI.
- [ ] Localization/content pipeline and package version/compatibility checks.
- [ ] Target player builds, dependency security/license checks, performance profiling.
- [ ] API/how-to docs, sample games, regression suite and first release tag.
- [ ] GUI editor only when core authoring UX and runtime are stable.

**Gate:** third-party creator can build/preview/package Story without changing Engine source.

## Guardrails

- **No Unity**; Rust-native foundation, Bevy candidate backend, Yarn dialogue adapter.
- **Headless core** must not depend on Bevy/Yarn/Rhai or Story packages.
- **No native hot-loaded Rust code** in ordinary Story packages.
- **No genre-specific systems in Engine**; use generic state/actions/views.
- **No unsupported rollback/sandbox guarantees**; prove safety and continuation first.
- **Do not spend first milestones building GUI editor/custom story language**; prove runtime + creator loop.

See [docs/architecture.md](./docs/architecture.md) for boundaries, runtime proposal and research gates.
