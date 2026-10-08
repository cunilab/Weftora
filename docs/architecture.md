---
id: WFT-ARCH-001
title: Weftora Architecture
type: architecture
doc_version: 0.6.0
status: proposed
implementation: not_started
created: 2026-10-08
updated: 2026-10-08
last_reviewed: null
owner: Weftora Maintainers
scope: windows-first
product_release: unreleased
related_pr: 1
supersedes: null
---

# Architecture — Rust-Native Weftora

**Product name:** Weftora. Repo: `cunilab/Weftora`. **First supported target:** Windows x64; cross-platform expansion gated.  
**Goal:** independent, creator-friendly alternative to Ren'Py-style narrative engines: **self-contained player/executable + separate Story packages**, built with generic Rust runtime and optional internal rendering/dialogue adapters.

## 1. Product identity and limits

Weftora is a **game engine and authoring platform**, not a game, a Unity extension, or a collection of hard-coded life-sim features. Design priorities:

1. **Story iteration without recompiling Rust:** change dialogue, data, supported gameplay rules, declarative UI, flow and assets; validate and relaunch/preview using prebuilt Engine tools. Creators do not write Rust.
2. **One stable Engine supports many games:** VN and non-VN narrative simulations share runtime/API.
3. **Rust core with enforceable boundaries:** typed contracts, explicit error handling, automated validation; no assumption compiler catches behavioral/spec drift.
4. **Practical authoring UX:** plain editable source, clear errors, quick preview, packaging and localization; editor follows working CLI and player. Game authors launch games directly; they do not embed a Rust library in game projects.
5. **Windows-first completion:** complete tested Windows player, two sample Stories and CLI/export before work on other OS targets; preserve portable core contracts now.

**Boundary of “Story can change anything”:** only behavior supported by current Engine API, data/flow language, script host and widgets. A new native renderer, shader integration, low-level input device, OS service or widget primitive requires a compiled adapter/Engine change. No arbitrary Rust code loading into release player.

### Standalone engine, not developer framework

**Product interface = apps**, not crates. Weftora owns its runtime, content pipeline, player, UX, and export process. Rust crates are internal implementation modules. End users should install Weftora, author Story folders, run/check them, and distribute runnable games **without** writing Rust, wiring Bevy systems, creating a Cargo app, or recompiling core.

**Deliverables (separate):**
1. **`weftora-player`:** compiled player/runtime loads **external, version-compatible Story package(s)** and runs game. No genre-specific source import or hardcoded Story file paths. Runtime can run independently with chosen Story path; initial MVP supports one active Story.
2. **`weftora` CLI:** proposed `new`, `check`, `run`, `export` commands; author and validate Story, launch same player, package for supported targets. CLI must not impose Rust/Cargo on Story authors; tooling may use Cargo internally for building Engine releases.
3. **`weftora-editor` (later):** visual authoring/preview for same Story schema/Engine runner. Editor is optional; text+CLI remains first-class.
4. **Story source/package:** human-editable Yarn/data/events/screens/assets; package manifest/capability contract. No `Cargo.toml`, Rust crate, `main.rs`, Bevy app or native DLL necessary for supported gameplay.

**Two runtime modes:** `weftora run path/to/story` plays development Story using prebuilt player; `weftora export path/to/story` validates/packs Story and copies target-specific prebuilt player + supported assets into independent distribution. Commands and details are proposed, not currently implemented. Player may load directories in dev and verified package archives in production; packaging format TBD.

**Key distinction:** Bevy is implementation backend for rendering/input/assets, **not** a user-facing dependency or author programming model. Yarn provides authored dialogue, **not** full game engine. Author interacts with Weftora's stable content schemas + Story APIs.

**Dynamic boundary:** changing ordinary Story rules/dialogue/screens/assets should not rebuild player. New native capabilities or unimplemented widget/renderer features require an Engine release; never promise unlimited mods or native code loading.

### Extension tiers

[ADR 0001](./adr/0001-rust-native-engine.md) compares proposed Rust-native path with Unity, Godot and Ren'Py; this stack is unproven until P2 acceptance.

- **Tier 1 — data:** Yarn, JSON/other selected text formats, assets, character/location/event definitions, themes.
- **Tier 2 — behavior:** declarative conditions/actions/events and later optional interpreted script with allowlisted host API.
- **Tier 3 — native:** Rust crates implementing new host services, renderer/UI widgets and platform integrations; build-time/release-time plugins, not content-only mods.

## 2. Architecture diagram

```text
                   Creator / CLI / Editor
                            |
                     validate / compile
                            |
                     Story Package(s)
     manifest | data | Yarn | flow | screen defs | assets
                            |
                        weftora-api
                            |
 +--------------------------+---------------------------+
 |               Rust runtime, headless                |
 | weftora-core: lifecycle/state/commands/signals          |
 | weftora-story: manifest/catalog/compat/loader           |
 | weftora-flow: predicates/triggers/sequence scheduler    |
 | weftora-save: persistence/checkpoints/migrations        |
 +--------------------------+---------------------------+
                            |
 +--------------------------+---------------------------+
 |            Optional backend/adapters                |
 | weftora-yarn | weftora-script (future) | weftora-render-bevy     |
 | weftora-ui-bevy | audio/input/asset adapters             |
 +--------------------------+---------------------------+
                            |
              weftora-player (composes crates)
                            |
                          OS / GPU
```

**Strict rule:** `weftora-api` and headless core must not depend on `bevy`, `yarnspinner`, `rhai`, a Story package, or frontend apps. Renderer/dialogue/script crates depend inward. `weftora-player` composes everything. A future different renderer must not force changes to state/flow/save semantics.

Bevy is recommended rendering/asset/input platform **candidate**; not project identity. Yarn is preferred dialogue authoring **candidate**; not universal scripting or state owner.

### Platform-neutral contract, Windows-first execution

- **Initial target:** Windows x64 (`x86_64-pc-windows-msvc`) and only Windows implementation/CI until Windows foundation acceptance. Repo is allowed to hold a **future** portability plan, not unfinished ports.
- **Core must not hardcode Windows:** Story paths are logical IDs/relative normalized package references, not Windows absolute paths; storage/location resolution is platform service; input actions are logical, not VK keys; lifecycle/window, clock, renderer, audio and asset IO abstracted at backend boundary.
- **Windows adapter now:** implement native event/window, keyboard/mouse and file/package paths, user-writable save location, audio/display scaling, player distribution; test real Windows GPUs.
- **Other adapters later:** Linux/macOS desktop, Android/iOS mobile and Web differ in storage/lifecycle/input/render/packaging/signing. Their existence does not justify implementing them before Windows works.
- **One Story schema:** optional target-specific processed asset bundles and platform metadata are permitted; never introduce OS-specific gameplay rules or require author-written Rust per target.
- **Portability validation now:** inspect boundaries/types in code review and headless tests. No Linux/macOS/mobile/Web build matrix until Windows release gate.
- [Platform support plan](./platforms.md) = scope, export matrix, gates and deferred risks.

## 3. Proposed Cargo workspace

```text
Weftora/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── crates/
│   ├── weftora-api/               # stable IDs, values, commands, traits
│   ├── weftora-core/              # headless kernel, lifecycle, state/signals
│   ├── weftora-story/             # manifest, catalogs, loader/validation
│   ├── weftora-flow/              # events/conditions/action scheduling
│   ├── weftora-save/              # slots, checkpoints, version/migrations
│   ├── weftora-yarn/              # optional Yarn compiler/runtime adapter
│   ├── weftora-script/            # optional interpreter (decision pending)
│   ├── weftora-render-bevy/      # visuals, assets, audio, input bridge
│   └── weftora-ui-bevy/          # generic UI widget renderer
├── apps/
│   ├── weftora-player/            # standalone executable loads Story package
│   ├── weftora-cli/               # author-facing 'weftora' CLI
│   └── weftora-editor/            # later, optional visual editor
├── examples/                    # roadmap tasks; not present in this PR
│   ├── hello-story/
│   └── life-sim/
├── tests/                     # package/golden/e2e fixtures
└── docs/
    └── architecture.md
```

Names are proposed. Weftora executable/CLI is what creators use; crates exist for Engine maintainers. Avoid proliferating crates before coherent APIs exist. Start with `weftora-api`, `weftora-core`, `weftora-story`, `weftora-player` and a minimal Bevy adapter; split only on real dependency boundaries. Cargo workspace directories for Stories are examples; **Story folders are content, not Cargo workspace members**. Published Story content must never become compile-time dependencies of core.

### Dependency direction

```text
Story files ──> weftora-api schema (serialized data/API contract)
                         ^
weftora-core / weftora-story / weftora-flow / weftora-save
                         ^
weftora-yarn / weftora-script / weftora-render-bevy / weftora-ui-bevy
                         ^
                    weftora-player
```

This shows logical dependencies; individual runtime crates may depend on `weftora-core` and/or `weftora-api` as needed. No reverse imports from headless crates into adapters, player, or example stories.

## 4. Runtime responsibility split

| Domain | Engine owns | Story owns |
| --- | --- | --- |
| State | typed storage, notifications, namespaces, persistence policy | keys, defaults, meaning, formulas |
| Commands | registry, argument validation, async/cancel, errors | rules such as travel, sleep, buy |
| Signals | dispatch, ordering, cleanup, loop guard | gameplay triggers and reactions |
| Flow | condition interpreter, event scheduler, action sequencing | event definitions, priority, conditions |
| Dialogue | adapter interface, node playback + UI callbacks | Yarn files, choices, custom Story actions |
| Screens | widget renderer, layout primitives, binding engine | screen trees, theme, actions, navigation |
| Visuals | sprite layers, transitions, animation primitives | characters, backgrounds, CG and choreography |
| Audio | channels, playback, fade, settings | music/voice cues, timing and files |
| Assets | stable ID resolver, cache, load/unload, errors | IDs, catalogs, asset files |
| Saves | atomic storage, schema versions, checkpoint/restore | versioned Story state + migrations |
| Tooling | validator, logging, content preview, debugger | authored packages and fixtures |

No engine-level `RelationshipSystem`, `TimeSystem`, `ShopSystem`, or `InventorySystem`. Games may ship reusable Story modules implementing such concepts.

## 5. Story package contract

Preferred dev format: human-editable directories. Release packaging can use a directory, archive or platform asset container decided by deployment spikes. IDs—not paths—form Story-facing references.

```text
stories/sample-vn/
├── manifest.json
├── data/             # characters, states, locations, catalogs
├── dialogue/         # Yarn source; compiled during build/validation
├── events/           # triggers/conditions/actions
├── screens/          # declarative widgets/bindings/actions
├── themes/
├── localization/
├── scripts/          # optional and disabled until interpreter accepted
└── assets/           # images, audio, fonts; licensed for distribution
```

Manifest v1 **proposal**, not supported implementation:

```json
{
  "schemaVersion": 1,
  "id": "sample.vn",
  "version": "0.1.0",
  "requiredEngineApi": "^0.1.0",
  "entrypoint": "intro",
  "catalogs": {
    "data": ["data/catalog.json"],
    "dialogue": ["dialogue/catalog.json"],
    "events": ["events/catalog.json"],
    "screens": ["screens/catalog.json"],
    "assets": ["assets/catalog.json"]
  },
  "capabilities": ["dialogue", "images", "audio", "screens"]
}
```

**Package rules:**
- Unique namespaced stable IDs; reject duplicates, missing IDs, unknown widgets/commands, invalid Yarn nodes and invalid conditions with source locations.
- Reject unsupported `schemaVersion`, API version/capabilities, malformed content and invalid entrypoint **before** gameplay.
- Normalize/validate file paths; reject traversal, absolute paths and oversized/unexpected inputs according to policy. Asset IDs are not a security boundary.
- Hash/version package outputs where useful; pack reproducibly; keep localized content and engine API compatibility explicit.
- Start with **one active Story**. Add dependency graphs/Story overlays only if tested; never silently merge namespace collisions.
- Story package may contain executable **interpreted scripts** only behind explicit opt-in. Never equate Story content with arbitrary Rust native library loading.
- Running same player executable against Story A or B requires compatible capabilities/build target. Assets may need target-specific processing.

## 6. Public API and game loop

Illustrative API vocabulary (not actual Rust signatures):

```text
state.get(id) / state.set(id, value) / state.add(id, number)
commands.execute(id, arguments) / commands.cancel(handle)
signals.emit(id, payload) / signals.subscribe(id, handler)
content.resolve(id) / assets.load(id)
views.show(id, options) / views.hide(id)
screens.open(id) / screens.close(id)
dialogue.start(node) / dialogue.choose(choice_id)
save.write(slot) / save.load(slot)
```

Runtime sequence:

```text
Input
  -> Story screen button / Yarn choice
  -> validated command
  -> Story-defined action/rule
  -> state mutation + signals
  -> eligible Story event conditions / ordered action sequence
  -> dialogue / view / screen / audio requests
  -> backend renders
```

Engine controls event-loop/reentrancy bounds; Story cannot recursively generate unbounded signal storms. Async commands require an explicit continuation/cancel model. Do not rely on implicit Bevy ECS ordering for headless narrative semantics.

## 7. State, Yarn and deterministic flow

### Story authored actions (provisional contract)

Commands are schema-validated Story data with namespaced IDs and typed args, not arbitrary Rust functions. Minimum allowed primitives: `state.set`, `state.add`, `signal.emit`, dialogue/view/audio actions, composed with typed conditions and deterministic order. Validate static refs/params before activating package. Execute sequentially; each completed state write is atomic. On first runtime failure, stop remainder and report failed action index; **previous writes are not implicitly rolled back**. Opt-in transactions would need a new API/ADR. Enforce recursion, action-count and async cancellation budgets. New native primitive requires Engine release.


Use **one canonical gameplay state** with typed values and optional schema defaults. Initial types: `bool`, integer, finite float, string; complex collections deferred. Keys namespaced, e.g. `world.day`, `character.alice.affection`; Engine never parses domain meaning.

Yarn variable storage must be a *mapped adapter view* onto canonical state (or use an explicitly synchronized shadow with conflict tests). Define conversion, missing defaults, commit timing, error reporting, and persistent/transient scopes before v1 save/load. Do not maintain independent unsynchronized Yarn state.

Event scheduler specifies ordered trigger evaluation, explicit priority/tie-break, action sequencing, async suspension, cancellation, and bounds for repeated/reentrant signals. Preserve ordering across headless tests and Bevy player.

Sample (concept only):

```text
screen ui.cafe -> onClick: command story.buy_coffee
story.buy_coffee -> validate funds -> state change -> signal.emit(coffee_bought)
event first_purchase -> when flag false -> dialogue.start(first_purchase)
Yarn choice -> update mapped state -> view.show(character.happy)
```

Commands like `story.buy_coffee` belong to Story, never `weftora-core`.

## 8. Declarative UI and presentation

Story defines *layout, widgets, bindings, actions and theme*, not Rust GUI code per screen. Engine provides generic primitives: panel, text, image, button, list, overlay, screen, focus, animation, input routing. Schema validated during `weftora-cli check`.

Example proposal:

```json
{
  "id": "ui.main_menu",
  "type": "screen",
  "children": [
    {
      "type": "button",
      "textKey": "menu.start",
      "onClick": {
        "command": "dialogue.start",
        "args": { "node": "intro" }
      }
    }
  ]
}
```

Binding design must define updates, validation, layout constraints, accessibility/focus, localization, scaling and UI state restoration. Native widget types require renderer/UI adapter additions. First UX target: show images + choices + menu + transitions, then themed custom screens.

## 9. Dialogue and script adapters

### Yarn compilation boundary

`weftora check`: compile Yarn source through version-pinned compiler/adapter; source-located diagnostics. `weftora run`: recompile changed development source. `weftora export`: package versioned compiled dialogue + source-map IDs; release player loads compiled artifact **without** requiring compiler, Rust or Cargo. P2 decides supported syntax, artifact format, cache invalidation, compiler/runtime version handshake and test cases. Early time-boxed P0 probe must precede freezing kernel dialogue/state APIs. If unsupported, fallback adapter must retain agreed Yarn author format or trigger new product ADR.


**Yarn:** test `yarnspinner` (standalone compiler/runtime) and optionally `bevy_yarnspinner` as Bevy integration. Vendor/wrap no Yarn types in public `weftora-api`. Yarn Spinner for Rust project currently labels itself *work in progress / no official support*; compatibility, compiling Yarn, running choices, custom commands, variable mapping, localization and save semantics require a gated spike. If unsupported, adapter replacement must not break Story package architecture.

**Scripting:** declarative Story events and commands first. Later prototype **Rhai** as optional interpreter for arithmetic/rules/functions only, not replacement for Yarn or flow scheduler. Do not promise sandboxed modding:
- allowlist host calls; no unrestricted file/network/OS access or reflection;
- set operation/call-depth/memory/object limits where supported; define cancellation/timeout strategy and host-side resource limits;
- map values/errors into typed state and diagnostics;
- test hostile input, panic handling, save compatibility and deterministic behavior.
- sandbox *effectiveness* is validation target, not guarantee inferred from choosing Rust.

**Why not custom story language now?** Creator UX improves more by reusing Yarn + typed declarative definitions initially. Re-evaluate custom syntax only with documented missing authoring use cases.

## 10. Persistence, replay and rollback

### Save activation and version policy

Independent `saveFormat` (integer), `engineApi` (compatible range), `storyId` and `storyVersion` fields. MVP defaults to exact saveFormat/Story version and Story ID; reject incompatible Engine API; exceptions only via tested explicit migrations. Pipeline: bounded read → metadata/compatibility check → migration in temporary snapshot → full state/checkpoint validation → atomic activation. Any failure retains previous active state and known-good on-disk save. Final version matrix is Phase 0 decision; implementation verified Phase 3.


Engine serializes slots, metadata, Story identity/version, schemas, chosen state scopes, Story module payloads, and checkpoints. Use atomic write/rename where supported, corruption detection, backup/restore and explicit migration failure.

```json
{
  "saveFormat": 1,
  "engineApi": "0.1.0",
  "storyId": "sample.vn",
  "storyVersion": "0.1.0",
  "state": {},
  "modules": {},
  "checkpoint": {}
}
```

MVP saves only at **explicit safe points** (idle boundaries, known Yarn node/choice boundaries after adapter verification). UI/view state reconstructed from serialized Story state plus checkpoint metadata. Exact continuation of arbitrary coroutines, in-flight commands, transitions, audio position, and rollback/replay is **not** promised. Incompatible Story/package versions require explicit migration or refusal; do not silently restore partial state.

## 11. Toolchain and drift protection

Rust provides strong *compile-time* memory/type safety, but neither proves correct game mechanics nor prevents AI-driven valid-code regressions. Treat boundaries as executable tests.

### Controls

- `#![forbid(unsafe_code)]` in headless/domain crates; any unavoidable unsafe in adapters requires a documented, reviewed exception and tests.
- Cargo workspace with pinned `rust-toolchain.toml`, committed `Cargo.lock`, dependency policy and optional license/advisory checks.
- CI dependency graph assertion: `weftora-core`, `weftora-api`, `weftora-story`, `weftora-flow`, `weftora-save` contain no Bevy/Yarn/Rhai/Story dependency.
- Validate manifest/JSON schemas from Rust-owned types where practical; revisioned schemas and fixtures.
- Headless golden tests (same Story input → same ordered events/state/requests); error-case tests (duplicate IDs, malformed commands, recursion cycles, invalid save).
- Property-based tests for serialization/idempotence where useful; integration tests with Story A/B on same player; snapshot/API compatibility diff review.
- Merge gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, plus package validator/architecture/target build checks as they exist.
- ADR for any change to dependency direction, exposed Engine capability, runtime language, file formats, renderer, or save semantics.

Sample CIs above are **planned**, not present/ran. Compile-time checks do not validate author-visible behavior.

**Measured completion:** mandatory P0–P7 test IDs, evidence and PASS/BLOCKED rules: [success criteria](./success-criteria.md). Descriptive architecture alone never satisfies release gate.

## 12. Rollout and acceptance gates

**Gate 0: architecture/bootstrap.** Cargo builds, zero reverse imports, Story manifest validates/rejects errors; initial example package.

**Gate 1: playable narrative slice.** Via prebuilt player/CLI (no author Rust/Cargo), single external Story with Yarn choices, two screens, image/CG, audio, persistent state; player runs without Engine source edits.

**Gate 2: dynamic rules.** Declarative Story conditions/actions and custom themed UI; changing rules/assets does not recompile Engine; optional scripting gated by spike.

**Gate 3: two distinct games.** VN package and life-sim-like package run against identical compiled player binary/revision; no Rust/Bevy game project per Story. Delete either package: Engine still compiles and other game runs.

**Gate 4: creator UX.** CLI generates, validates, runs and exports player+Story for supported targets without author Rust/Cargo; preview/reload during development; useful source-located errors and debugger.

**Gate 5: release safety.** Verified **Windows x64** release executable on clean Windows machine, CLI/export, two Stories and full regressions; clean licenses/deps, save mismatch/corruption coverage, runtime error recovery and architecture CI. Only then unlock non-Windows ports.

## 13. Open ADRs / research gates

1. **Windows x64 only** through foundation release; other targets after release gate. Define adapter boundaries early, defer portability implementation/CI. See [platform plan](./platforms.md).
2. Pin compatible Rust/Bevy/Yarn versions after sample compile; Bevy 0.19 with yarnspinner 0.9 is a **candidate** combination, not tested in this repo.
3. Choose Bevy UI vs alternative immediate-mode UI for declarative screen renderer.
4. Choose Story distribution container/content compiler; path/packaging semantics across platforms.
5. Decide Yarn variable mapping, nested state types and safe-checkpoint limitations via runtime tests.
6. Evaluate Rhai needs, constraints and adversarial-test results before selecting scripting interface.
7. Define save/schema semver ranges and migration tooling; eventually rollback model.
8. Decide license after dependency/license audit.

## Reference projects (research; not added deps)

- [Bevy](https://bevyengine.org/) — rendering/asset/input candidate.
- [Yarn Spinner for Rust](https://github.com/YarnSpinnerTool/YarnSpinner-Rust) — WIP compiler/runtime + Bevy adapter; upstream version table.
- [Rhai](https://rhai.rs/) — optional embedded scripting candidate.

No Rust code, tools, executable binaries, or tests are claimed by this proposal.

## Change History

| Date | Version | Change | Reference |
| --- | --- | --- | --- |
| 2026-10-08 | 0.6.0 | Align example paths, declare action/compile/save behavior and Rust decision ADR. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.5.1 | Standardize metadata/header and doc lifecycle. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
