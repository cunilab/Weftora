# Visual Game Engine

A reusable Unity foundation for visual novels, narrative games, and life-sim games.

> **Engine = reusable technology. Story = replaceable game logic, flow, data, and assets.**

## Stack

- Unity 6 LTS
- C#
- Yarn Spinner
- Free/open-source dependencies where practical

## Architecture

```text
Story
  ├── Rules
  ├── Flow
  ├── Yarn
  ├── Data
  ├── UI
  └── Assets
      ↓
Engine
  ├── State
  ├── Commands
  ├── Signals
  ├── Modules
  ├── Content / Assets
  ├── Presentation
  ├── UI / Audio
  ├── Yarn Adapter
  ├── Save / Load
  └── Debug / Validation
      ↓
Unity
```

The Engine must never depend on Story.

## Main Rule

If a feature can reasonably change between games, it belongs in **Story**.

Examples:

- relationship rules
- time rules
- inventory
- economy
- locations
- events
- jobs
- shops
- quests
- phone systems
- characters and dialogue

The Engine only provides generic mechanisms those systems can use.

## Documentation

- [PRD](./PRD.md)
- [Roadmap](./ROADMAP.md)

## Status

Early foundation and architecture stage.
