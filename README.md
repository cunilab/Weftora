# Weftora

Weftora is a standalone engine for narrative and story-driven 2D games: visual novels, life-sims, and anything in between. It is written in Rust, but **game authors never write Rust**. They write Yarn dialogue, data, rules and screens, and run them on a prebuilt player.

> **Status:** Phase 0 walking skeleton is implemented; see the [verification evidence](./docs/evidence/phase-0.md). Clean Windows export evidence (P0-10) is deferred to [issue #5](https://github.com/cunilab/Weftora/issues/5). [Phase 1 headless checks](./docs/evidence/phase-1.md) pass after review fixes; the implementation is merged in [PR #7](https://github.com/cunilab/Weftora/pull/7). Phase 2 is in progress, beginning with compiled dialogue and a compiler-free player.

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
