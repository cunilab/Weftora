---
id: WFT-ARCH-001
title: Weftora Architecture
status: proposed
---

# Weftora Architecture

This document describes **how** Weftora is built. Product scope is in the [PRD](../PRD.md); phase order is in the [roadmap](../ROADMAP.md). Nothing here is implemented yet; all names and formats are proposals.

## 1. Product boundary

Weftora's product is its **apps** — `weftora-player` and the `weftora` CLI — not its crates. Authors install the apps, write Story folders and ship games. They never create a Cargo project, wire Bevy systems or recompile the Engine.

"A Story can change anything" means anything the current Engine API supports. Three extension tiers make that boundary explicit:

| Tier | What | Who | Needs Engine release? |
| --- | --- | --- | --- |
| 1 — Content | Yarn dialogue, data, catalogs, screens, themes, assets | Author | No |
| 2 — Behaviour | Declarative events/actions and Rhai scripts ([ADR 0003](./adr/0003-story-rules-language.md)) | Author / designer | No |
| 3 — Native | New widgets, renderer features, platform services, Engine primitives | Engine maintainer | Yes |

Story packages never load native code.

## 2. Crates

```text
                      Story package (files)
          manifest · data · Yarn · events · scripts · screens · assets
                               │  loaded and validated by
┌──────────────────────────────┴──────────────────────────────┐
│ Headless (no Bevy, no Yarn, no Rhai, no Story)               │
│   weftora-api    IDs, values, command/signal/capability types│
│   weftora-core   lifecycle, state, commands, signals         │
│   weftora-story  manifest, catalogs, loader, path policy     │
│   weftora-flow   conditions, events, action scheduler        │
│   weftora-save   save container, checkpoints, migrations     │
└──────────────────────────────┬──────────────────────────────┘
                               │  depended on by
┌──────────────────────────────┴──────────────────────────────┐
│ Adapters                                                     │
│   weftora-yarn         Yarn compiler/runtime (yarnspinner)   │
│   weftora-script       Rhai host                             │
│   weftora-render-bevy  window, views, audio, input, assets   │
│   weftora-ui-bevy      declarative UI renderer               │
└──────────────────────────────┬──────────────────────────────┘
                               │  composed by
            apps/weftora-player · apps/weftora-cli
```

**Start small.** Phase 0 creates only `weftora-api`, `weftora-core`, `weftora-story`, `weftora-yarn`, `weftora-render-bevy` and the two apps. `weftora-flow`, `weftora-save`, `weftora-script` and `weftora-ui-bevy` are split out only when their code exists and a real dependency boundary justifies them.

### Dependency rules

- Headless crates must not depend, directly or transitively, on `bevy`, `yarnspinner`, `rhai`, any adapter, any app or any Story. CI enforces this from `cargo metadata`.
- Adapters depend inward on headless crates, never the reverse.
- Only the apps compose adapters.
- Public `weftora-api` types never expose Bevy, Yarn or Rhai types.
- Ordering and semantics of narrative flow are defined and tested in headless code; never rely on Bevy system scheduling order for them.

## 3. Responsibility split

| Area | Engine owns | Story owns |
| --- | --- | --- |
| State | typed storage, scopes, notifications, serialisation | keys, defaults, meaning, formulas |
| Commands | registry, argument validation, async/cancel, errors | game commands such as `travel`, `sleep`, `buy` |
| Signals | dispatch, ordering, recursion limit | which signals exist and who reacts |
| Flow | condition evaluation, event scheduling, action sequencing | event definitions, priorities |
| Scripts | Rhai host, allowlisted API, limits | rule scripts |
| Dialogue | Yarn adapter, node playback, state bridge | Yarn files, Yarn commands used |
| UI | widget renderer, layout, bindings, focus | screens, theme, navigation |
| Visuals | layers, sprites, transitions | characters, backgrounds, CGs, staging |
| Audio | channels, fades, volume settings | cues and files |
| Assets | ID resolution, cache, load/unload, errors | catalogs and files |
| Saves | atomic storage, container format, load pipeline | save schema, checkpoints, migrations |
| Tooling | validator, logs, debugger, exporter | content and test fixtures |

The Engine has no `RelationshipSystem`, `TimeSystem`, `ShopSystem` or `InventorySystem`. Stories build those.

## 4. Story package

Development format is a plain folder. The release container (folder or archive) is decided in Phase 6.

```text
examples/hello-story/
├── manifest.json
├── data/          characters, locations, initial state, catalogs
├── dialogue/      Yarn source
├── events/        trigger / condition / action definitions
├── scripts/       Rhai rule scripts (Phase 4+)
├── screens/       declarative UI
├── themes/
├── localization/
└── assets/        images, audio, fonts (must be licensed for redistribution)
```

Manifest v1 proposal (values are illustrative):

```json
{
  "manifestSchema": 1,
  "id": "com.example.hello-story",
  "storyVersion": "1.2.1",
  "requires": {
    "engineApi": ">=0.2.0 <0.3.0",
    "capabilities": ["dialogue.yarn/1", "ui.basic/1"]
  },
  "saveSchema": 2,
  "entrypoint": "intro",
  "catalogs": {
    "data": ["data/catalog.json"],
    "dialogue": ["dialogue/catalog.json"],
    "events": ["events/catalog.json"],
    "screens": ["screens/catalog.json"],
    "assets": ["assets/catalog.json"]
  }
}
```

Version fields are defined in [ADR 0002](./adr/0002-versioning-compatibility.md). Phase 0 uses `manifestSchema: 0`, which is explicitly unstable.

**Loader rules**

- IDs are namespaced and unique; duplicates, missing references, unknown commands/widgets and invalid Yarn nodes are rejected with file and location.
- Unsupported manifest schema, Engine API range or capability is rejected **before** any Story state becomes active. Activation is all-or-nothing.
- Paths are relative to the Story root. The loader rejects absolute, UNC, drive-relative and `\\?\` paths; `..` traversal; Windows reserved device names with or without an extension (`CON`, `NUL.txt`, `COM1`, …); names ending in a dot or space; `:` (alternate data streams); names that collide case-insensitively; symlinks and junctions that resolve outside the root; and packages over the file-count, file-size or total-size limits. Asset IDs are not a security boundary — the path policy is.
- One active Story at a time. Story dependencies or overlays need their own design.

## 5. Runtime model

### State

One canonical game state. Keys are namespaced strings (`world.day`, `character.alice.affection`); the Engine never interprets them.

- Phase 1 value types: `bool`, `int` (i64), `float` (finite f64), `string`.
- Phase 4 adds `list` and `map` (string keys) of those values, required by Story B ([ADR 0003](./adr/0003-story-rules-language.md)).
- Every key has a scope: `persistent` (saved), `session` (not saved) or `settings` (saved separately from progress).
- Serialisation is deterministic (stable key order) so golden tests and saves are reproducible.

### Commands and actions

A command has a namespaced ID, a typed argument schema and returns success or a typed error. Engine primitives include `state.set`, `state.add`, `signal.emit`, `dialogue.start`, `view.show`, `view.hide`, `audio.play`, `audio.stop`, `screen.open`, `screen.close`, `save.write`.

Stories define their own commands as either an ordered list of actions (declarative) or a Rhai function. Execution semantics:

- Static references and argument types are validated when the package loads.
- Actions run in order; each state write is atomic.
- On the first failure the remaining actions are skipped and the error reports the failing action's index. **Earlier writes are not rolled back.** Transactions, if ever needed, require a new ADR.
- Recursion depth, action count per dispatch and async cancellation are bounded by the Engine.

### Signals and flow

Signals are delivered in a documented deterministic order. Nested emits are queued, not delivered re-entrantly, and a per-frame budget stops signal storms with an explicit error.

Events are `trigger → conditions → actions` with an explicit priority and tie-break (declaration order). The scheduler, not Bevy, decides evaluation order, so headless tests and the real player behave identically.

```text
input → screen button / Yarn choice → command → actions or Rhai
      → state changes + signals → matching events → dialogue / view / audio requests
      → renderer
```

## 6. Dialogue (Yarn Spinner)

- Use the **Bevy-independent `yarnspinner` crates** inside `weftora-yarn`. Do not use `bevy_yarnspinner`: it ties Weftora's Bevy upgrades to a third-party crate's release schedule. The Rust port is community-maintained and labelled work-in-progress; Phase 0 tests it and records the fallback.
- Fallback options, in order: patch or fork the Rust port; bundle the official Yarn Spinner command-line compiler with the CLI and run its compiled output on the Rust runtime (output compatibility must be verified first); switch dialogue language (new ADR).
- Yarn variables are a view onto canonical state, not a separate store. Missing defaults and write timing are specified and tested.
- Variable names (provisional, Phase 0): Yarn `$name` maps to state key `name`; `__` inside a Yarn name stands for `.` in the key, so `$world__day` is `world.day`.
- Type mapping: Yarn (Rust port) has only `bool`, `string` and a 32-bit float `number`. `bool` and `string` map directly; `float` maps to `number`; `int` maps to `number` only within ±2^24 and reading a larger value from Yarn is a Story error, not silent rounding. Lists and maps are **not** exposed to Yarn; Stories expose scalar views of them through Yarn functions backed by declarative actions or Rhai.
- Before Phase 4, Yarn commands map to Engine primitive commands only. From Phase 4, Story-defined commands (declarative or Rhai) can also be called from Yarn.
- `weftora check` compiles Yarn and reports errors with file and node. `weftora run` recompiles changed files. `weftora export` ships compiled dialogue with its own `dialogueArtifactFormat` version, compiler identity and content hash. The player never contains the compiler and refuses unknown artifact formats. (Phase 0 shortcut: the player compiles Yarn at startup; P2-03 removes this.)

## 7. Rules scripting (Rhai)

Rhai is embedded for rules too complex for declarative actions; the reasons are in [ADR 0003](./adr/0003-story-rules-language.md). This section is the host contract:

- **Allowlisted API only:** read and write state, execute commands, emit signals, and a deterministic random source whose seed is stored in canonical state.
- **Nothing else reachable:** build the engine with `Engine::new_raw()` plus only the needed packages; no file module resolver and `set_max_modules(0)` (no `import`); `eval` disabled; the `no_time` feature enabled; no file, network, process or reflection access.
- **Limits:** operations per call, call depth, string, array and map sizes. Exceeding a limit is a Story error, not a crash or hang.
- **No hidden script state:** scripts keep nothing between calls; anything persistent lives in canonical state, so saves stay complete.
- **Diagnostics:** `weftora check` compiles every script; syntax and runtime errors report Story file and line.

This makes accidents safe; it is **not** a guarantee that hostile mods are safe.

## 8. UI and presentation

Stories define screens as data: layout, widgets, theme, bindings to state and actions. Initial widgets: screen, panel, text, image, button, list, choice list, overlay. Bindings must define update timing, validation, focus order, localisation and scaling. New widget types are Tier 3 (Engine release).

Presentation layers: background, character, CG, overlay, UI. Operations: show, hide, position, scale, opacity, fade, move, crossfade, shake.

Whether the declarative UI renders through Bevy UI or another library is an open question decided in Phase 2.

Illustrative screen definition (format provisional):

```json
{
  "id": "ui.main_menu",
  "type": "screen",
  "children": [
    {
      "type": "button",
      "textKey": "menu.start",
      "onClick": { "command": "dialogue.start", "args": { "node": "intro" } }
    }
  ]
}
```

## 9. Saves

Header fields: `saveFormat`, `storyId`, `storyVersion`, `saveSchema`, `checkpoint.id`, plus `writerEngineApi` and `writerEngineRelease` for diagnostics. Which of these decide compatibility is defined in [ADR 0002](./adr/0002-versioning-compatibility.md#compatibility-rules).

```json
{
  "saveFormat": 1,
  "writerEngineApi": "0.2.0",
  "writerEngineRelease": "0.3.0-alpha.1",
  "storyId": "com.example.hello-story",
  "storyVersion": "1.2.1",
  "saveSchema": 2,
  "checkpoint": { "id": "scene.intro.after_choice" },
  "state": {}
}
```

- Saves happen only at **safe points**: no command, transition or async action is in flight, and the dialogue is at a known node or choice boundary. In-flight animations, audio position and coroutines are not saved; the view is rebuilt from state plus checkpoint.
- Write: serialise → write temp file → flush → atomic rename; keep the previous save as a backup.
- Load: bounded read → header compatibility → migrate a temporary copy if a migration is registered → validate state and checkpoint → atomic activation. On failure the current game and the on-disk save are untouched.
- Saves live in the platform's user data directory, never inside the Story package.

## 10. Drift protection

- `#![forbid(unsafe_code)]` in headless crates. Any `unsafe` in adapters needs a reviewed, documented exception.
- Pinned `rust-toolchain.toml` and committed `Cargo.lock`; licence and advisory checks (`cargo deny`) from Phase 0.
- **Merge gate (the single list; other docs link here):** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`, the dependency-direction check, and the docs link/header check.
- Headless golden tests: same Story input → same ordered events, state and render requests.
- Negative fixtures for every loader, validator and save rule.
- Contract changes need an ADR; see [standards](./standards.md#changing-documents).

Passing compilation never proves author-visible behaviour; the [success criteria](./success-criteria.md) define what does.

## 11. Open questions

1. **Authoring file format.** JSON is shown above but is verbose and has no comments. Candidates: JSON with comments (JSONC), TOML, RON or YAML. Decide in Phase 0 with the first real Story files; schema validation is generated from Rust types in any case.
2. **UI renderer.** Bevy UI vs. an alternative for the declarative screen renderer (Phase 2).
3. **Release package container.** Folder vs. archive (Phase 6).
4. **Yarn ↔ state mapping details**, including list/map values (Phases 2 and 4).
5. **Licence**, after the dependency licence audit (Phase 6).
6. **Exact dependency versions.** Phase 0 builds against Bevy 0.20.0 and `yarnspinner` 0.9.0 (crates.io, October 2026; `yarnspinner` has Bevy off by default). They are confirmed or replaced by the Phase 0 spikes and pinned in Phase 2 (P2-07).

## References

- [Bevy](https://bevyengine.org/)
- [Yarn Spinner for Rust](https://github.com/YarnSpinnerTool/YarnSpinner-Rust)
- [Rhai](https://rhai.rs/)
