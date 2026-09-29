# Visual Game Engine

A reusable Unity foundation for visual novels, narrative games, and life-simulation games.

The project is designed around one strict separation:

> **Engine = reusable technology. Story = replaceable game logic and content.**

The Engine should know how to load content, store state, execute commands, present visuals, play audio, run Yarn dialogue, and save data. It should **not** know what a relationship, job, shop, bedroom, character route, inventory, or chapter means.

Those concepts belong to the Story layer.

---

## Status

**Early foundation / architecture stage.**

The current repository is defining the engine/story boundary before implementation begins.

See the full product requirements document:

- [PRD.md](./PRD.md)

---

## Goals

- Build a small reusable Unity runtime instead of a game-specific framework.
- Keep story rules and gameplay rules easy to change.
- Use **Yarn Spinner** for dialogue and narrative scripting.
- Keep dependencies free, open source, or included with Unity whenever practical.
- Allow Story packages to define their own systems without modifying Engine code.
- Make assets replaceable through stable IDs instead of hard-coded file paths.
- Support fast iteration with validation, logging, and debugging tools.
- Make save/load generic enough to persist Story-defined state.
- Keep the public Engine API small and stable.

---

## Core Architecture

```text
                     STORY
                       │
         ┌─────────────┼─────────────┐
         │             │             │
       Rules          Flow         Content
         │             │             │
         └─────────────┼─────────────┘
                       ▼
────────────────────────────────────────────
                  ENGINE API
────────────────────────────────────────────
 State       Commands       Signals
 Modules     Content        Assets
 Views       UI             Audio
 Dialogue    Save           Input
────────────────────────────────────────────
                       │
                       ▼
                     UNITY
```

Dependency direction is always:

```text
Story ──► Engine ──► Unity
```

The Engine must never depend on Story.

---

## Engine Responsibilities

The Engine provides generic technical capabilities.

### Runtime

- application lifecycle;
- bootstrap;
- service registration;
- module hosting;
- structured logging.

### State

Generic typed state storage:

```text
player.money = 500
world.day = 5
alice.relationship = 20
flags.met_alice = true
```

The Engine stores these values but does not interpret their meaning.

### Commands

Generic command execution:

```text
state.set
state.add
signal.emit
view.show
view.hide
audio.play
dialogue.start
save.write
```

Story modules may register higher-level commands such as:

```text
travel
sleep
buy_item
change_relationship
```

### Signals

Decoupled communication between systems:

```text
Subscribe(...)
Emit(...)
Unsubscribe(...)
```

### Conditions

Generic expression evaluation:

```text
player.money >= 500
alice.relationship >= 20
flags.met_alice == true
```

The Engine understands comparison and boolean logic, not game-specific concepts.

### Content and Assets

Stable content IDs:

```text
character.alice
character.alice.happy
location.bedroom
cg.alice.date01
audio.bgm.home
```

Story references IDs. The Engine resolves them to Unity assets.

### Presentation

Generic visual operations:

```text
Show
Hide
Fade
Move
Scale
Shake
CrossFade
SetLayer
SetPosition
```

### UI

Reusable UI hosting:

- screens;
- panels;
- overlays;
- modals;
- popups;
- notifications;
- dialogue host.

Game-specific interfaces such as a phone, shop, map, or stats screen belong to Story.

### Audio

Technical playback for:

- BGM;
- ambience;
- SFX;
- voice;
- volume groups;
- fading and crossfading.

### Dialogue

Yarn Spinner integration for:

- starting Yarn nodes;
- exposing state;
- invoking Engine commands;
- registering Story commands/functions;
- dialogue lifecycle events.

### Persistence

Generic save infrastructure:

- save slots;
- serialization;
- metadata;
- timestamps;
- backups;
- versioning;
- migration hooks.

The Engine saves Story state without understanding its meaning.

### Development Tools

- state inspector;
- content registry inspector;
- command runner;
- signal inspection;
- validation;
- debug console;
- runtime logging.

---

## Story Responsibilities

Everything specific to an actual game belongs to Story.

Examples:

- characters;
- relationships;
- time rules;
- economy;
- inventory rules;
- locations;
- activities;
- jobs;
- shops;
- quests;
- routes;
- chapters;
- events;
- progression;
- phone systems;
- Yarn dialogue;
- UI theme and layout;
- character sprites;
- backgrounds;
- CGs;
- music;
- SFX;
- balance values.

A Story package may create modules such as:

```text
TimeModule
RelationshipModule
InventoryModule
LocationModule
EconomyModule
EventModule
ShopModule
PhoneModule
JobModule
```

These are intentionally **not** part of the Engine core.

---

## Example

A Story may contain:

```text
alice.relationship = 18
player.money = 300
world.hour = 17
```

And Yarn may contain:

```text
title: AliceCafe
---

Alice: Want to get something to drink?

-> Sure
    <<travel "cafe">>
    <<change_relationship "alice" 2>>

-> Maybe later
    Alice: Okay.

===
```

The Engine knows how to:

- store values;
- run Yarn;
- execute registered commands;
- emit signals;
- display assets.

The Story defines what `travel` and `change_relationship` actually do.

---

## Planned Project Structure

```text
Assets/
├── Engine/
│   ├── Runtime/
│   │   ├── Core/
│   │   ├── State/
│   │   ├── Commands/
│   │   ├── Signals/
│   │   ├── Conditions/
│   │   ├── Modules/
│   │   ├── Content/
│   │   ├── Assets/
│   │   ├── Presentation/
│   │   ├── UI/
│   │   ├── Audio/
│   │   ├── Dialogue/
│   │   ├── Persistence/
│   │   └── Input/
│   │
│   ├── Editor/
│   │   ├── Validation/
│   │   └── DebugTools/
│   │
│   └── VisualGameEngine.Runtime.asmdef
│
└── Story/
    ├── Runtime/
    │   ├── Rules/
    │   ├── Modules/
    │   └── Flow/
    │
    ├── Data/
    │   ├── Characters/
    │   ├── Locations/
    │   ├── Items/
    │   └── Events/
    │
    ├── Yarn/
    ├── UI/
    ├── Assets/
    │   ├── Characters/
    │   ├── Backgrounds/
    │   ├── CG/
    │   ├── Audio/
    │   └── UI/
    │
    └── VisualGameEngine.Story.asmdef
```

Assembly rule:

```text
VisualGameEngine.Story
          │
          ▼
VisualGameEngine.Runtime
```

The reverse dependency is prohibited.

---

## Planned Engine API

The API is still conceptual, but the intended direction is:

```csharp
Engine.State.Get<T>(key);
Engine.State.Set(key, value);

Engine.Commands.Execute(id, args);

Engine.Signals.Emit(id, payload);
Engine.Signals.Subscribe(id, handler);

Engine.Modules.Get<T>();

Engine.Content.Resolve(id);
Engine.Assets.Load<T>(id);

Engine.Views.Show(id, options);
Engine.Views.Hide(id);

Engine.Audio.Play(id);
Engine.Audio.Stop(id);

Engine.Dialogue.Start(node);

Engine.Save.Save(slot);
Engine.Save.Load(slot);

Engine.UI.Open(id);
Engine.UI.Close(id);
```

Story modules build their own APIs on top of these primitives.

---

## Technology Direction

### Required

- **Unity 6 LTS**
- **C#**
- **Yarn Spinner for Unity**

### Dependency Policy

Prefer:

1. Unity built-in packages;
2. free/open-source libraries;
3. small dependencies with clear value.

Avoid adding packages simply because they might be useful later.

---

## Development Roadmap

### Phase 0 — Repository Foundation

- Unity project setup
- Engine and Story assemblies
- Yarn Spinner
- coding conventions
- Git LFS where needed

### Phase 1 — Kernel

- bootstrap/lifecycle
- state store
- command bus
- signal bus
- module registration
- logging

### Phase 2 — Content and Presentation

- content registry
- asset resolver
- view/image presentation
- UI host
- audio service

### Phase 3 — Yarn

- Yarn adapter
- generic command bridge
- Story command registration
- dialogue lifecycle signals

### Phase 4 — Persistence

- save container
- save slots
- Story state serialization
- versioning
- migration hooks

### Phase 5 — Story Flow

- generic condition evaluation
- reference Story flow/event module
- data-driven triggers/actions

### Phase 6 — Tooling

- state inspector
- content validation
- command runner
- registry inspector
- debug tools

---

## Foundation Completion Target

The first foundation is considered complete when a replacement Story package can:

1. register its own modules;
2. define arbitrary state keys;
3. register commands;
4. emit and consume signals;
5. register and resolve content;
6. present its assets;
7. run Yarn dialogue;
8. call Story-specific behavior from Yarn;
9. evaluate Story-defined conditions;
10. save and restore Story state;
11. use validation and debugging tools;

without modifying Engine code.

---

## Design Rule

When deciding whether a feature belongs in Engine or Story, ask:

> **Could another game reasonably not need this feature, or need completely different rules for it?**

If the answer is **yes**, it normally belongs in **Story**.

| Capability | Owner |
|---|---|
| State storage | Engine |
| Command execution | Engine |
| Signal bus | Engine |
| Content/asset loading | Engine |
| Generic presentation | Engine |
| Generic UI hosting | Engine |
| Audio playback | Engine |
| Save infrastructure | Engine |
| Yarn integration | Engine |
| Time rules | Story |
| Characters | Story |
| Relationships | Story |
| Inventory rules | Story |
| Economy | Story |
| Locations | Story |
| Events | Story |
| Dating | Story |
| Jobs | Story |
| Shops | Story |
| Quests | Story |
| Phone | Story |

---

## Documentation

- [Product Requirements Document](./PRD.md)

More architecture and implementation documentation will be added as the foundation is built.

---

## License

A project license has not been selected yet.
