# Weftora

Weftora is a standalone engine for narrative and story-driven 2D games: visual novels, life-sims, and anything in between. It is written in Rust, but **game authors never write Rust**. They write Yarn dialogue, data, rules and screens, and run them on a prebuilt player.

> **Status:** Phase 0 walking skeleton is implemented; see the [verification evidence](./docs/evidence/phase-0.md). Clean Windows export evidence (P0-10) is deferred to [issue #5](https://github.com/cunilab/Weftora/issues/5). [Phase 1 headless checks](./docs/evidence/phase-1.md) pass after review fixes; the implementation is merged in [PR #7](https://github.com/cunilab/Weftora/pull/7). Phase 2 implementation is merged. [Phase 3](./docs/evidence/phase-3.md) is in progress with safe checkpoint saves; native Windows manual results remain maintainer-owned in issue #5.

## How it is meant to work

```text
Author edits a Story folder (Yarn, data, screens, images, audio)
        │
   weftora check     → validate content, compile dialogue, report errors with file/line
        │
   weftora run       → play it on the prebuilt Weftora player
        │
   weftora export    → standalone game folder: prebuilt player + Story + compiled dialogue
```

- **Engine** — the compiled player and tools: state, commands, signals, rules execution, rendering, UI, audio, saves, validation, export.
- **Story** — everything that makes a particular game: dialogue, characters, rules, events, screens, themes, assets.
- Changing a Story never requires rebuilding the Engine. One player binary runs many different games.

## Targets

Windows x64 is the first and only release target until the foundation release. Development also works on macOS and Linux, but they are not supported releases. See the [platform plan](./docs/platforms.md).

## Repository layout

```text
crates/     Engine libraries (internal to Weftora)
apps/       weftora-player, weftora CLI
examples/   sample Stories (content, not Rust)
docs/       architecture, decisions, criteria
```

## Documentation

- [Product requirements](./PRD.md): what Weftora must do and what it won't do.
- [Roadmap](./ROADMAP.md): phases, starting with a playable skeleton.
- [Architecture](./docs/architecture.md): crates, boundaries and the Story package format.
- [Success criteria](./docs/success-criteria.md): pass/fail checks for each phase.
- [Decisions (ADRs)](./docs/adr/): why Rust, how versioning works, and the rules language.
- [Docs index](./docs/README.md)

## Preview and packaged dialogue

`weftora check <story-dir>` compiles sources and validates the artifact. Use `weftora run <story-dir>` for previews: it compiles into a temporary folder, launches the player, and removes the artifact after the player exits. Editing dialogue still needs no player rebuild.

`weftora export <story-dir> --out <empty-dir>` writes `story/`, `compiled/dialogue.json`, the player and `weftora-export.json`. The player loads the artifact without a compiler. Run an exported player without arguments; for validation without a window, use `weftora-player <export-dir>/story --validate`. A direct source-folder launch requires `--dialogue <artifact>`; otherwise use the CLI preview. Modified exported Yarn sources are rejected as stale until exported again.

## Checkpoint saves (Phase 3)

A save-enabled Story declares positive saveSchema and the save/1 capability. Place a literal checkpoint command at the beginning of a Yarn node, before any executable statement:

    title: Introduction
    ---
    <<checkpoint scene.introduction>>
    Welcome back.
    ===

The first line or choices in that node is its save point. Progressing past it closes the point. The opening must reconstruct the complete scene; checkpoint nodes cannot use random functions or read Session variables. Keep IDs stable across compatible dialogue/image edits, and bump saveSchema when checkpoint meaning changes. See [ADR 0007](./docs/adr/0007-safe-checkpoint-saves.md).

For deterministic headless checks:

    weftora run <story-dir> --headless --save-at scene.introduction
    weftora run <story-dir> --headless --load --choices 0

Save stops at the eligible display and emits a saved record. Load validates and replays the opening before emitting gameplay events. Optional --save-dir <directory> selects an exact test directory outside the Story. Without it, the host user-data location is used: LOCALAPPDATA/Weftora/saves/Story-ID on Windows, ~/Library/Application Support/Weftora/saves/Story-ID on macOS, or XDG_DATA_HOME/Weftora/saves/Story-ID (otherwise ~/.local/share) on Linux. One save.json slot and the previous save.bak are maintained; corrupt or incompatible loads never overwrite either.

## Story A: Last Signal

[Story A](./examples/story-a/README.md) is a complete short example with a title menu, two routes, characters, CG, music, transitions, themed choices and checkpoint saves. Its CC0 rectangles and tones are prototype assets. Hello Story remains the unchanged earlier test fixture.

Use the [author tutorial](./docs/story-a-tutorial.md) to edit dialogue, screens/theme and an image with prebuilt tools. Windows CI packages baseline and two edited variants with the same player hash. [Native Windows and independent-author steps](./docs/evidence/phase-3-windows.md) remain in issue #5 with blank results.

The [commands demo](./examples/commands-demo/README.md) exercises typed Story commands from Yarn, events and screen buttons, including checkpoint save replay. Native Windows checks remain queued in issue #5.

The [collections demo](./examples/collections-demo/README.md) stores a flat list inventory and map schedule, with whole-value command edits and checkpoint replay.

The [scripts demo](./examples/scripts-demo/README.md) uses a bounded Rhai command for inventory/schedule edits, callable from Yarn, events and screen buttons.

The [random demo](./examples/random-demo/README.md) keeps a seeded dice stream in canonical state and continues the same sequence after checkpoint load.

The [queries demo](./examples/queries-demo/README.md) reads collection length and membership from Yarn through scalar-only helpers.

The [screens demo](./examples/screens-demo/README.md) replaces layouts, colors and navigation using existing widgets, with a repeatable same-player UI edit proof.

[Market Day (Story B)](./examples/life-sim/README.md) is a short life-sim with Home/Market, scheduled work, energy/coins, inventory and replayable seeded pay on the same Engine.
