# Visual Game Engine

**Rust-native, story-driven game engine** for visual novels, narrative games, and life simulations. Goal: alternative authoring/runtime experience inspired by Ren'Py and Yarn Spinner—not a Unity project or a wrapper around a single game.

**Engine = stable, reusable Rust technology. Story = replaceable content, gameplay rules, dialogue, UI, events, data, and assets.**

## Architecture

```text
Story source (Yarn, JSON, assets, optional rules)
                 |
            validate / build
                 |
            Story package
                 |
          Public Story API
                 |
  Rust core: state, flow, save, assets,
    commands, signals, UI contract
                 |
    Adapters: Bevy / Yarn / scripts
                 |
          Native game player
```

- **Core:** renderer-agnostic Rust crates; no Bevy, Yarn, or Story package dependency.
- **Runtime:** loads compatible Story packages; Engine never imports game-specific behavior.
- **Rendering:** Bevy adapter is preferred candidate, not a dependency of core.
- **Dialogue:** Yarn Spinner for Rust behind adapter; early compatibility spike required.
- **Rules:** declarative flow first; Rhai is optional, subject to security/platform tests.
- **Tools:** Rust CLI for validation/packing/testing, then creator-oriented preview/editor.

Target: ship **the same compiled Engine/player revision with two distinctly different Story packages**, without editing Engine source. New native primitives may still require compiled extensions.

## Documents

- [Architecture](./docs/architecture.md) — boundaries, modules, contracts, packaging, runtime design.
- [PRD](./PRD.md) — product requirements and testable success criteria.
- [Roadmap](./ROADMAP.md) — implementation order and acceptance gates.

## Status

Architecture proposal only. No Cargo workspace, compiled engine, or runtime tests committed yet. Exact Rust, Bevy, Yarn and scripting versions/target platforms must be verified and pinned in Phase 0.
