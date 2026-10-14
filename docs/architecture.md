---
id: WFT-ARCH-001
title: Weftora Architecture
status: proposed
---

# Weftora Architecture

This document describes **how** Weftora is built. Product scope is in the [PRD](../PRD.md); phase order is in the [roadmap](../ROADMAP.md). This is the target architecture; verified implementation results are recorded in [Phase 0 evidence](./evidence/phase-0.md). Later-phase contracts remain proposals.

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
│   weftora-yarn         Yarn runtime + compiled artifacts   │
│   weftora-yarn-compiler  authoring-only Yarn compiler         │
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
├── manifest.jsonc
├── data/          characters, locations, initial state, catalogs
├── dialogue/      Yarn source
├── events/        trigger / condition / action definitions
├── scripts/       Rhai rule scripts (Phase 4+)
├── screens/       declarative UI
├── themes/
├── localization/
└── assets/        images, audio, fonts (must be licensed for redistribution)
```

Story files other than Yarn are JSONC: JSON with comments and trailing commas ([ADR 0004](./adr/0004-story-authoring-format.md)).

Manifest v1 (Phase 1):

```jsonc
{
  "manifestSchema": 1,
  "id": "com.example.hello-story",
  "storyVersion": "0.1.0",
  "title": "Hello Story",
  "requires": {
    "engineApi": ">=0.1.0 <0.2.0",
    "capabilities": ["dialogue.yarn/1"]
  },
  "entrypoint": "Start",
  "dialogue": ["dialogue/intro.yarn"],
  "assets": {"bg.room": "assets/room.png"},
  "state": {"world.day": {"default": 1, "scope": "persistent"}},
  "initialState": {"world.day": 2},
  "catalogs": {"data": ["data/items.jsonc"]}
}
```

Version fields and comparator syntax are defined in [ADR 0002](./adr/0002-versioning-compatibility.md). The current Engine API is `0.1.0`; the supported capability is `dialogue.yarn/1`. Legacy schema 0 remains loadable with its original fields; new fields require schema 1. Save and compiled-dialogue fields arrive in Phases 3 and 2 respectively.

Phase 1 data catalogs are JSONC arrays of records with a non-empty string `id`. IDs are unique across all files in the category. Duplicate JSON object keys are errors, including duplicate asset IDs. Catalog diagnostics name the file and JSON pointer (for example `/2/id`); parse errors include a line. Duplicate IDs name both declaration locations. Missing lookups use the caller's file and location; Phase 1 exposes this as a lookup API, without inventing a general content-reference schema. Other catalog categories arrive with the features that use them.

**Loader rules**

- IDs are namespaced and unique; duplicates, missing references, unknown commands/widgets and invalid Yarn nodes are rejected with file and location.
- Unsupported manifest schema, Engine API range or capability is rejected **before** any Story state becomes active. Activation is all-or-nothing.
- Paths are relative to the Story root. The loader rejects absolute, UNC, drive-relative and `\\?\` paths; `..` traversal; Windows reserved device names with or without an extension (`CON`, `NUL.txt`, `COM1`, …); names ending in a dot or space; `:` (alternate data streams); names that collide case-insensitively; symlinks and junctions that resolve outside the root; and packages over the limits: 4,096 files, 8,192 entries including directories, 16 MiB per file, 256 MiB total. Inventory validates every package entry before reading the manifest; text reads enforce the per-file limit again. Case-collision checks use Unicode uppercase spelling conservatively. Asset IDs are not a security boundary — the path policy is.
- Symlinks and junctions inside a Story are rejected even when their targets remain inside the root. A user-selected root may itself be a symlink. References must match canonical on-disk spelling, including case and separators; repeated `/` separators are rejected. Unreferenced dotfiles are counted by inventory but omitted from export. This deliberately includes editor lock files and nested `.git` contents: their links or size can reject a package, so keep working-copy metadata outside the Story root. All Windows reparse points are rejected; cloud placeholders and deduplicated files are not supported by this directory loader.
- One active Story at a time. Story dependencies or overlays need their own design.

## 5. Runtime model

### Lifecycle

`weftora-core::lifecycle` starts services in registration order during `boot → load`, then enters `ready`. Only `ready` may enter `play`. Shutdown releases services in reverse order. Failed initialization first tears down the failing service's partial resources, then earlier services; later services never start. Cleanup continues after a teardown error and returns the collected errors. Failure ends in `shutdown`, never a half-ready runtime; restart requires a new lifecycle. Explicit shutdown is idempotent, and dropping a live lifecycle also tears down services. Services own their resources and must make their cleanup safe after a failed init.

The headless CLI registers one `story` service. It owns validated Story/dialogue content, canonical state, the shared built-in command registry, signal observers and the dialogue runner, in that initialization order; teardown releases them in reverse. A failed start clears all of them before returning an error. The core still supports multiple services with ordered rollback; the Bevy player retains its Phase 0 startup until presentation integration in Phase 2.

### State

One canonical game state. Keys are namespaced strings (`world.day`, `character.alice.affection`); the Engine never interprets them.

- Phase 1 value types: `bool`, `int` (i64), `float` (finite f64), `string`.
- Phase 4 adds `list` and `map` (string keys) of those values, required by Story B ([ADR 0003](./adr/0003-story-rules-language.md)).
- Every key has a scope: `persistent` (saved), `session` (not saved) or `settings` (saved separately from progress).
- `state` declares typed defaults and scopes; `initialState` applies overrides after defaults. Legacy initial keys and missing Yarn defaults infer a persistent declaration on their first write. Once declared, a key's type cannot change, including after removal. `reset` restores the default; `remove` returns `Result<Value, StateError>` and reports a missing key explicitly. JSON `2` and `2.0` infer distinct int and float declarations; `set` must preserve that type, while numeric `add` permits an int amount on a float key.
- Mutations enqueue notifications in write order with key, scope, old value and new value; no-op and rejected writes emit nothing. Consumers drain the queue at their update boundary. State construction does not emit notifications. CLI validation and the dialogue runner preflight all Yarn declaration names and scalar defaults before initialization, because the upstream runtime only logs storage errors. Invalid declarations cannot partially populate canonical state. The headless runtime also clears valid Yarn default-fill notifications before entering play; it then drains changes at each dialogue event and emits `state.changed` signals with the changed key as their string payload. Scope snapshots and iteration have stable key order; no save container is introduced yet. Whole-State equality includes declarations and pending notifications; compare scope snapshots when only values matter.
- Serialisation is deterministic (stable key order) so golden tests and saves are reproducible.

### Commands and actions

A command has a namespaced ID, a typed argument schema and returns success or a typed error. Engine primitives include `state.set`, `state.add`, `signal.emit`, `dialogue.start`, `view.show`, `view.hide`, `audio.play`, `audio.stop`, `screen.open`, `screen.close`, `save.write`.

The Phase 1 registry validates scalar arguments before creating work and validates result types, including finite floats. Handlers return standard Rust futures; the caller drives polling. Cancelling a pending task drops its future immediately, releasing owned resources; it does not undo earlier writes. Completion and cancellation are terminal and repeat polls return the same result. This is cooperative cancellation, not background threads or an executor.

Stories define their own commands as either an ordered list of actions (declarative) or a Rhai function. Execution semantics:

- Static references and argument types are validated when the package loads.
- Actions run in order; each state write is atomic.
- On the first failure the remaining actions are skipped and the error reports the failing action's index. **Earlier writes are not rolled back.** Transactions, if ever needed, require a new ADR.
- Recursion depth, action count per dispatch and async cancellation are bounded by the Engine.

### Signals and flow

Signals deliver to subscribers in registration order. Handlers return nested signals, which are appended to a FIFO queue after the current signal; there is no re-entrant delivery. Subscription tokens allow unsubscribe between dispatches. The caller supplies a per-dispatch budget: the initial emission, every handler delivery and every nested emission consume one unit. Exhaustion reports an explicit error and discards pending emissions. Handler errors also discard pending emissions; earlier state writes remain. The shared-state runtime holds one write guard during dispatch. Handlers use the supplied mutable State and must not reacquire that same SharedState lock. Headless trace observers consume real state-change signals; Story-defined handlers remain Phase 4 work. The renderer may allocate a per-frame budget across dispatches later.

Events are `trigger → conditions → actions` with an explicit priority and tie-break (declaration order). The scheduler, not Bevy, decides evaluation order, so headless tests and the real player behave identically.

```text
input → screen button / Yarn choice → command → actions or Rhai
      → state changes + signals → matching events → dialogue / view / audio requests
      → renderer
```

## 6. Dialogue (Yarn Spinner)

- Use the **Bevy-independent `yarnspinner` crates** inside `weftora-yarn`. Do not use `bevy_yarnspinner`: it ties Weftora's Bevy upgrades to a third-party crate's release schedule. `weftora-yarn-compiler` contains source compilation and is used only by the CLI; the player depends on `weftora-yarn` plus direct core/runtime crates. A separate crate prevents workspace feature unification from pulling compilation into the player. The Rust port is community-maintained and labelled work-in-progress; Phase 0 tests it and records the fallback.
- Fallback options, in order: patch or fork the Rust port; bundle the official Yarn Spinner command-line compiler with the CLI and run its compiled output on the Rust runtime (output compatibility must be verified first); switch dialogue language (new ADR).
- Yarn variables are a view onto canonical state, not a separate store. Missing defaults and write timing are specified and tested.
- Variable names (provisional, Phase 0): Yarn `$name` maps to state key `name`; `__` inside a Yarn name stands for `.` in the key, so `$world__day` is `world.day`.
- Type mapping: Yarn (Rust port) has only `bool`, `string` and a 32-bit float `number`. `bool` and `string` map directly; `float` maps to `number`; `int` maps to `number` only within ±2^24 and reading a larger value from Yarn is a Story error, not silent rounding. Lists and maps are **not** exposed to Yarn; Stories expose scalar views of them through Yarn functions backed by declarative actions or Rhai.
- Before Phase 4, Yarn commands map to Engine primitive commands only. From Phase 4, Story-defined commands (declarative or Rhai) can also be called from Yarn.
- `weftora check` compiles Yarn and reports errors with file and node. `weftora run` recompiles changed files. `weftora export` ships compiled dialogue with its own `dialogueArtifactFormat` version, compiler identity and content hash. The player never contains the compiler and refuses unknown artifact formats. Format 1 and its loading checks are defined in [ADR 0002](./adr/0002-versioning-compatibility.md#compiled-dialogue-format-1).

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

Illustrative screen definition:

```jsonc
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

1. ~~Authoring file format~~ — decided: JSONC ([ADR 0004](./adr/0004-story-authoring-format.md)).
2. **UI renderer.** Bevy UI vs. an alternative for the declarative screen renderer (Phase 2).
3. **Release package container.** Folder vs. archive (Phase 6).
4. **Yarn ↔ state mapping details**, including list/map values (Phases 2 and 4).
5. **Licence**, after the dependency licence audit (Phase 6).
6. **Exact dependency versions.** Phase 0 builds against Bevy 0.20.0 and `yarnspinner` 0.9.0 (crates.io, October 2026; `yarnspinner` has Bevy off by default). They are confirmed or replaced by the Phase 0 spikes and pinned in Phase 2 (P2-07).

## References

- [Bevy](https://bevyengine.org/)
- [Yarn Spinner for Rust](https://github.com/YarnSpinnerTool/YarnSpinner-Rust)
- [Rhai](https://rhai.rs/)
