---
id: WFT-PRD-001
title: Weftora Product Requirements
status: proposed
---

# Weftora Product Requirements

Related: [Architecture](./docs/architecture.md) · [Roadmap](./ROADMAP.md) · [Success criteria](./docs/success-criteria.md) · [Platforms](./docs/platforms.md)

## 1. Vision

Weftora is a standalone engine for narrative and 2D story-heavy games. It is written in Rust but **authors never write Rust**. The inspiration is Ren'Py's end-to-end workflow and Yarn Spinner's approachable dialogue.

Weftora ships as two prebuilt programs:

- **the player** (`weftora-player`), which runs a game, and
- **the CLI** (`weftora`), which creates, checks, runs and exports games.

A game is a **Story package**: a folder of dialogue, data, rules, screens and assets. Changing a Story never requires rebuilding the player. Only genuinely new low-level capabilities (a new widget type, a new platform service) require a new Engine release.

Weftora is not a Unity extension, not a Yarn renderer, and not a Rust/Bevy library that developers embed in their own game code.

## 2. Users

- **Author** — installs prebuilt tools, writes Yarn dialogue and Story data, previews and exports a game. Gets actionable error messages. Never installs Rust, Cargo or Bevy.
- **Game designer** — expresses game rules (events, conditions, resources, schedules) through declarative data and, where needed, Rhai scripts inside the Story.
- **Player** — runs an exported game with reliable choices, visuals, audio and saves.
- **Engine maintainer** (human or AI agent) — changes the Rust Engine safely behind tests and dependency rules.

## 3. Goals

1. **One player, many games.** The same compiled player binary runs two meaningfully different Stories (a visual novel and a life-sim) without Engine changes.
2. **Fast Story iteration.** Authors change dialogue, data, rules, screens and assets, then check and run them with the prebuilt tools.
3. **Headless core.** State, flow, rules and save logic build and test without Bevy, Yarn or any Story.
4. **Clear authoring errors.** Every content error points to a file and location.
5. **Reliable saves.** Saves load across compatible Story and Engine updates, and fail safely otherwise.
6. **Maintainable Rust.** Typed contracts, enforced dependency direction, CI and golden tests.
7. **Open dependencies.** Permissively licensed, pinned and audited.
8. **Windows release first, portable design.** Windows x64 is the only release target until the foundation release; core contracts stay platform-neutral.

## 4. Non-goals for the foundation

- Unity integration of any kind.
- 3D worlds, physics, multiplayer or networking.
- Built-in genre systems in the Engine (dating, economy, quests, phone, inventory, calendars). Stories build these.
- A replacement for the Yarn language.
- Loading native code (Rust/C/C++ DLLs) from Story packages.
- A guaranteed sandbox for untrusted third-party mods.
- Rollback, or saving in the middle of running commands or animations.
- A GUI editor before the CLI and player are proven.
- Release builds for any platform other than Windows x64.

## 5. Requirements

Details of each mechanism live in the [architecture](./docs/architecture.md); pass/fail checks live in [success criteria](./docs/success-criteria.md).

**R1. Story packages.** The player loads one external Story package chosen at launch. The package declares its identity, required Engine API range and capabilities. Invalid packages are rejected as a whole, before gameplay, with file and location in the error. Story file paths are confined to the package root.

**R2. Lifecycle.** Boot → load → ready → play ⇄ pause → shutdown run in a documented order; failed initialisation leaves no half-loaded Story.

**R3. State.** One canonical, typed, namespaced game state. Values: bool, integer, finite float, string, and — before Story B — lists and maps of those. Defaults, change notifications and explicit persistence scopes. The Engine never interprets key meaning.

**R4. Commands and signals.** Commands have stable IDs, typed arguments, results and errors, and may be asynchronous and cancellable. Signals have deterministic ordering and bounded recursion. Stories define their own commands from Engine primitives.

**R5. Rules and flow.** Stories define events as trigger → conditions → ordered actions with priorities. Simple rules are declarative data; complex rules use Rhai scripts with host-enforced limits ([ADR 0003](./docs/adr/0003-story-rules-language.md)). No genre rule lives in Engine code.

**R6. Dialogue.** Yarn Spinner is the dialogue language. Authors get compiler errors with file and node during `check`; exported games ship compiled dialogue and do not need the compiler. Yarn variables read and write the canonical state, never a separate copy.

**R7. Presentation and audio.** Backgrounds, multiple character sprites, CGs, layers, positioning, fades and transitions. Audio channels for music, sound effects, ambience and voice, with fades and user volume.

**R8. Declarative UI.** Stories define screens, widgets, layout, theme, state bindings and actions as validated data. The initial widget set grows from real Story needs. New widget types need an Engine release.

**R9. Assets.** Stories reference assets by stable ID through catalogs, never by raw path in game logic. Missing and duplicate IDs are reported clearly.

**R10. Saves.** Saves are written only at safe points, atomically, to a user-writable location. Loading validates and, if needed, migrates a temporary copy before replacing live state; a failed load never damages the current game or the existing save. Compatibility follows [ADR 0002](./docs/adr/0002-versioning-compatibility.md): a different Story version alone never rejects a save.

**R11. Creator tools.** `weftora new | check | run | export`. Restart-based preview first; hot reload later where state rules allow it.

**R12. Distribution.** `export` copies a prebuilt player plus the validated Story into a standalone game folder and records the player, compiler and content identities. Mismatched tool versions are refused.

**R13. Versioning.** Engine release, Engine API, manifest schema, Story release, save container format, Story save schema and compiled dialogue format are versioned independently ([ADR 0002](./docs/adr/0002-versioning-compatibility.md)).

**R14. Quality gates.** Every merge passes the gate defined in [architecture §10](./docs/architecture.md#10-drift-protection). Headless crates forbid `unsafe`. Golden tests cover narrative and flow behaviour. Compiling is never accepted as proof of behaviour.

**R15. Performance.** Provisional, unmeasured Windows budgets: cold start to menu ≤ 5 s; input-to-feedback p95 ≤ 100 ms; idle memory ≤ 750 MiB per sample Story; `weftora check` on the starter Story ≤ 2 s. Phase 2 measures a baseline and confirms or revises these by ADR.

## 6. Ownership rule

If behaviour could differ between two games, it belongs to the **Story**. If a generic technical primitive is missing, add it to the **Engine** deliberately, with tests, a version bump and, for contract changes, an ADR.

| Engine | Story |
| --- | --- |
| Lifecycle, state storage, command/signal/flow execution | Variables and their meaning, progression, rules |
| Rendering, audio, input, UI hosting | Dialogue, events, characters, locations |
| Package loading, validation, diagnostics | Screens, themes, localisation |
| Save technology | Assets, save schema and migrations |

## 7. Foundation acceptance

The foundation is complete when the Phase 6 gate in [success criteria](./docs/success-criteria.md) passes, which means:

- **Story A**, a visual novel, and **Story B**, a life-sim, both run on the same Windows release player binary.
- A person who does not know Rust can create, edit, check, run and export a game on a clean Windows machine.
- Invalid content, incompatible versions and corrupt saves produce actionable errors and never a partial load.
- Deleting every Story leaves the Engine workspace building and its tests passing.

## 8. Decisions

- Stack and go/no-go gates: [ADR 0001](./docs/adr/0001-rust-native-engine.md).
- Versioning and save compatibility: [ADR 0002](./docs/adr/0002-versioning-compatibility.md).
- Rules language and collection state: [ADR 0003](./docs/adr/0003-story-rules-language.md).
- Still open: UI renderer approach, Story authoring file format, release package container, licence. See [architecture §11](./docs/architecture.md#11-open-questions).
