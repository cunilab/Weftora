# Visual Game Engine

Reusable Unity runtime for visual novels, narrative games, and life sims. Build one Engine; load different Story packages without changing Engine source.

**Engine = reusable technical capabilities. Story = replaceable game behavior, flow, UI definitions, data, and assets.**

## Architecture

~~~text
Story authoring (Yarn, data, UI, optional scripts)
                   |
             validate / pack
                   |
              Story package
                   |
Engine runtime + adapters + public API
                   |
                  Unity
~~~

- **Engine:** state, commands, signals, content/asset loading, rendering, UI host, input, audio, persistence, diagnostics.
- **Story:** dialogue, events, gameplay rules, screens/themes, characters, locations, assets, localization.
- **Adapters:** Yarn integration and future optional scripting engines; no game-specific rules in Engine.
- **Extensions:** new native capabilities require compiled Engine/plugin code; Story may configure or script capabilities exposed by APIs.

No reverse dependency from Engine to Story. Yarn handles narrative scripting, not entire application runtime.

## Stack

- Unity 6 LTS (pin exact editor version during project bootstrap).
- C# for Engine/runtime.
- Yarn Spinner (free/open source), behind optional adapter.
- JSON/package manifests; Unity ScriptableObjects allowed for authoring.
- Optional sandboxed scripting runtime only after an evaluated prototype.

## Docs

- [PRD](./PRD.md) — requirements and acceptance.
- [Architecture](./ARCHITECTURE.md) — boundaries, runtime, package contract, script/UI/persistence model.
- [Roadmap](./ROADMAP.md) — ordered implementation slices.

## Status

Architecture proposal / no Unity implementation in repository yet.
