# Weftora

**Standalone Rust-native story game engine.** Ren'Py-like creation workflow, Yarn-style dialogue, replaceable game packages. **Not framework** requiring game authors to write Rust or integrate Bevy.

**Development target: Windows x64 first.** Stabilize entire Engine → Story → player → CLI → export pipeline before Linux, macOS, Android, iOS, or Web ports. Maintain platform-neutral core contracts from start.

**Engine = compiled reusable player/runtime. Story = external game data, dialogue, rules, flow, UI, themes, assets.**

## How creators use it

~~~text
Author edits Story files (Yarn, JSON, sprites/audio)
                      |
                 weftora check
                      |
                weftora run
                      |
         Prebuilt Weftora Engine/player
                      |
                Game playable
                      |
               weftora export
                      |
     Standalone distribution: player + Story
~~~

CLI commands above = **planned interface**, not implemented. Creators should not need Cargo/Rust toolchain or Engine recompilation to build supported Story content. Editor/visual preview later sit atop same player and package contract.

## Engine / Story boundary

- **Engine:** typed state, generic commands/signals, flow scheduler, asset loading, rendering/UI primitives, input/audio, save/load, validation, packaging.
- **Story:** data, gameplay logic, characters, dialogue, events, choices, screens/themes, asset catalogs, localization.
- **Adapters:** internal Bevy renderer + optional Yarn runtime + future scripted-rule adapter. Story authors need not depend on Bevy, Yarn Rust APIs, or Rust crates.
- **Native extension:** genuinely new low-level capability needs Engine/adapter code and release; ordinary Story changes must not.
- **Distribution:** one compatible compiled Engine/player can launch different Story packages; export bundles existing target player with Story files. Supported target builds must exist; no arbitrary platform support promised.

## Rust workspace (proposed)

```text
crates/       # Weftora Engine core, contracts, adapters
apps/         # player, CLI, future editor
stories/      # sample Stories; never compiled into Engine core
docs/         # architecture and decisions
```

Rust = implementation language for **engine maintainers**. Story = separate authoring format; no game-specific Rust compile step.

## Docs

- [Architecture](./docs/architecture.md) — standalone engine vs framework, runtime/player and Story package contracts.
- [PRD](./PRD.md) — product reqs and acceptance.
- [Roadmap](./ROADMAP.md) — Windows-first implementation phases and later port gates.
- [Platform plan](./docs/platforms.md) — Windows baseline, deferred Linux/macOS/Android/iOS/Web, platform boundaries and tests.

## Status

**Docs-only proposal.** No Rust workspace, playable engine, CLI, exporter or tests committed yet. Current repo: `cunilab/Weftora`. Windows x64 prioritized; other platforms not implemented or validated.
