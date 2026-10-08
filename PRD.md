# Product Requirements Document — Visual Game Engine

**Repository:** `cunilab/visual-game-engine`  
**Status:** Architecture proposal / Draft v0.2  
**Primary runtime:** Unity 6 LTS  
**Dialogue system:** Yarn Spinner (free / open source)  
**Purpose:** Define the boundary between a reusable technical engine and a replaceable, data-driven story/game package.

---

## 0. Scope Clarification (v0.2)

This project aims for a **Ren'Py-like reusable authoring/runtime platform on Unity**, not only a library of VN UI services. Engine is compiled generic runtime; Story packages provide replaceable rules, flow, UI definitions, dialogue, assets and data. See [ARCHITECTURE.md](./ARCHITECTURE.md) for authoritative proposed runtime/package boundaries and [ROADMAP.md](./ROADMAP.md) for current phase sequence.

**Dynamic does not mean unlimited:** Story may change behavior supported by public Engine APIs, data, declarative UI/actions and a future evaluated interpreted script adapter. Native Unity capabilities, engine extensions and new widget primitives may still require compiled code.

**MVP:** One full playable VN Story, then second fundamentally different Story using same Engine revision. Package contract, shared State↔Yarn mapping and safe save checkpoints precede advanced scripting/editor features.

## 1. Product Vision

Build a reusable visual-game foundation for narrative and life-simulation games where:

- the **Engine** contains technical, reusable runtime capabilities;
- the **Story** contains game-specific rules, flow, data, dialogue, UI content, and assets;
- most gameplay changes can be made without editing engine code;
- new stories or even substantially different games can reuse the same engine;
- story content can be iterated quickly without creating a large number of custom MonoBehaviours;
- Yarn Spinner is used for dialogue and narrative scripting, while Unity remains the runtime and presentation platform.

The central design principle is:

> **Engine = generic technology. Story = replaceable game behavior and content.**

The engine must not know what concepts such as "Alice", "relationship", "money", "bedroom", "dating", "job", "inventory", or "chapter" mean. Those concepts belong to the Story package.

---

## 2. Goals

### 2.1 Primary Goals

1. Create a small, reusable engine kernel.
2. Keep game-specific logic outside the engine.
3. Make story flow, rules, dialogue, data, and assets easy to replace.
4. Support Yarn Spinner as the default narrative layer.
5. Support data-driven conditions, commands, state, and signals.
6. Provide generic presentation primitives for visual-novel and 2D narrative games.
7. Provide reliable save/load infrastructure without coupling saves to specific story concepts.
8. Provide strong debugging and validation tools for content-heavy development.
9. Allow the same engine to host different Story packages.
10. Keep dependencies free, open source, or included with Unity whenever practical.

### 2.2 Success Criteria

The architecture is successful when:

- deleting the Story package does not break compilation of the Engine assembly;
- the Engine assembly contains no references to named story characters, locations, routes, chapters, or game-specific stats;
- a new Story package can define different game rules without editing Engine code;
- designers can change most story behavior by changing Yarn, data, configuration, assets, or Story modules;
- save/load works with Story-defined state;
- runtime errors from missing IDs and invalid content are reported clearly;
- a small sample Story can prove the full content pipeline.

---

## 3. Non-Goals

The first foundation is **not** intended to provide:

- a full RPG framework;
- a built-in dating system;
- a built-in economy;
- a built-in inventory implementation;
- a built-in quest system;
- a built-in life-sim calendar;
- a built-in shop system;
- a built-in phone system;
- a visual node editor for every system;
- multiplayer/networking;
- procedural 3D world systems;
- a custom replacement for Unity;
- a custom replacement for Yarn Spinner.

These may exist later as optional Story modules or separate packages.

---

## 4. Architecture Overview

```text
Unity
  │
  ▼
Engine / Kernel
  ├── Lifecycle
  ├── State Store
  ├── Command Bus
  ├── Signal Bus
  ├── Condition / Expression Evaluation
  ├── Module Host
  ├── Content Registry
  ├── Asset Loading
  ├── Presentation Primitives
  ├── UI Host
  ├── Audio
  ├── Yarn Adapter
  ├── Persistence
  ├── Debugging
  └── Validation
  │
  ▼
Story Package
  ├── Rules
  ├── Flow
  ├── Story Modules
  ├── Yarn
  ├── Characters
  ├── Locations
  ├── Events
  ├── Items
  ├── UI Content / Theme
  ├── Balance / Configuration
  └── Assets
```

Dependency direction is one-way:

```text
Story  ─────► Engine ─────► Unity
```

The Engine must never depend on Story.

---

## 5. Engine Responsibilities

The Engine provides **generic mechanisms**, not game-specific meaning.

### 5.1 Runtime Lifecycle

The Engine shall provide a predictable application lifecycle.

Minimum states:

- Boot
- Loading
- Main Menu
- Gameplay
- Dialogue
- Paused
- Transition
- Shutdown

The lifecycle service must allow systems and Story modules to initialize and shut down in a deterministic order.

Example conceptual lifecycle:

```text
Boot
→ initialize engine services
→ load Story manifest
→ register Story modules
→ load persistent settings
→ initialize Yarn
→ load/start game
```

---

### 5.2 Generic State Store

The Engine shall provide a typed key/value runtime state store.

Minimum supported value types:

- bool
- int
- float
- string

Example keys:

```text
player.money
world.day
world.hour
alice.relationship
flags.met_alice
ship.fuel
kingdom.reputation
```

The Engine does not interpret these keys.

Required operations:

```text
Get(key)
Set(key, value)
Add(key, value)
Remove(key)
Exists(key)
Reset(key)
```

Desirable later:

- lists;
- maps;
- scoped state;
- default values;
- schema validation;
- state change history for debugging.

---

### 5.3 Command System

The Engine shall expose a generic command-execution mechanism.

Core engine commands should be technical primitives, for example:

```text
state.set
state.add
signal.emit
wait
view.show
view.hide
audio.play
audio.stop
dialogue.start
content.load
save.write
save.load
```

Story-specific commands such as:

```text
travel
sleep
buy_item
increase_relationship
go_to_work
start_date
```

must be implemented in the Story layer.

Requirements:

- commands are addressable by stable IDs;
- commands validate parameters;
- synchronous and asynchronous commands are supported;
- commands return useful errors;
- Story modules can register additional commands.

---

### 5.4 Signal / Message Bus

The Engine shall provide a lightweight event bus so systems do not require direct references to one another.

Required operations:

```text
Subscribe(signalId, handler)
Unsubscribe(signalId, handler)
Emit(signalId, payload)
```

The Engine treats signal IDs as opaque strings or IDs.

Story examples:

```text
day_started
location_changed
player_slept
item_purchased
relationship_changed
```

Engine examples:

```text
save_completed
content_loaded
view_opened
dialogue_started
```

---

### 5.5 Condition and Expression Evaluation

The Engine shall provide a generic way to evaluate state-based conditions.

Minimum operators:

- `==`
- `!=`
- `>`
- `>=`
- `<`
- `<=`
- AND
- OR
- NOT

Examples:

```text
player.money >= 500
alice.relationship >= 20
world.hour >= 18
flags.met_alice == true
```

The evaluator must not contain special knowledge about money, relationships, time, characters, locations, or inventory.

Later extensions may include:

- custom functions;
- collection membership;
- Story-registered predicates;
- parsed expressions;
- editor validation.

---

### 5.6 Module Host

The Engine shall support optional Story modules.

Conceptual interface:

```csharp
public interface IGameModule
{
    void Initialize();
    void Shutdown();
}
```

Possible Story modules:

```text
TimeModule
RelationshipModule
InventoryModule
LocationModule
EconomyModule
ShopModule
PhoneModule
JobModule
EventModule
```

The Engine only manages module lifecycle and service registration. It does not provide the rules of those systems.

---

### 5.7 Content Registry

The Engine shall provide a central registry for Story-defined content.

The registry should support stable IDs for content such as:

```text
character.alice
location.bedroom
item.ramen
event.alice.date01
cg.alice.date01
audio.bgm.home
ui.phone
```

Required capabilities:

- register content by ID;
- resolve content by ID;
- reject duplicate IDs;
- report missing IDs clearly;
- allow validation before entering gameplay.

The registry must remain agnostic about the story meaning of the content.

---

### 5.8 Asset Resolver and Loading

Story content should reference stable asset IDs rather than arbitrary filenames.

Example:

```text
character.alice.happy
bg.bedroom.day
cg.alice.date01
audio.music.home
```

The Engine is responsible for resolving these IDs to actual Unity assets.

Initial implementation may use direct Unity references or ScriptableObjects. The API should allow future migration to Addressables without changing Story logic.

Responsibilities:

- resolve IDs;
- load assets;
- cache assets where appropriate;
- unload assets where appropriate;
- report missing assets;
- support sprite/image, audio, prefab, and generic Unity object resolution.

---

### 5.9 Presentation Primitives

The Engine shall provide generic presentation services.

Minimum image/view operations:

```text
Show
Hide
SetPosition
SetScale
SetLayer
SetOpacity
FadeIn
FadeOut
CrossFade
Move
Shake
```

The Engine may provide generic presentation layers such as:

```text
Background
Character
CG
Overlay
UI
```

The Story decides what content is shown and why.

The Engine must not contain a hard-coded "Alice presenter", "dating CG", "bedroom system", or similar story-specific presentation class.

---

### 5.10 UI Host

The Engine shall provide reusable UI infrastructure rather than game-specific screens.

Generic concepts:

- screen;
- panel;
- modal;
- overlay;
- popup;
- list;
- notification;
- menu host;
- dialogue host.

Story-defined screens may include:

- phone;
- shop;
- inventory;
- stats;
- character profile;
- map;
- job UI.

Those screens and their rules belong to Story.

---

### 5.11 Audio Service

The Engine shall provide technical audio playback.

Capabilities:

- BGM playback;
- SFX playback;
- ambience playback;
- voice channel support;
- volume groups;
- fade/crossfade;
- stop/pause/resume;
- persistent user volume settings.

The Engine does not decide which song belongs to which story scene.

---

### 5.12 Yarn Spinner Adapter

Yarn Spinner is the default dialogue system.

The Engine shall provide an adapter between Yarn and engine primitives. State shared with Yarn must have a single source of truth, explicit type mapping, and a defined persistence lifecycle.

Responsibilities:

- start a Yarn node;
- expose Engine state to Yarn where appropriate;
- expose generic Engine commands to Yarn;
- allow Story modules to register Story-specific Yarn commands/functions;
- forward dialogue lifecycle events through the signal bus;
- keep Yarn-specific implementation details behind an adapter where practical.

Example Story Yarn:

```text
Alice: Want to go out?

-> Yes
    <<travel "cafe">>

-> Stay home
    Alice: Okay.
```

`travel` is not an Engine command. It is registered by a Story module.

---

### 5.13 Persistence Infrastructure

The Engine owns save-file technology. The first delivery supports explicit safe checkpoints; exact in-flight Yarn/async/animation continuation and rollback remain out of scope until proved.

The Story owns the meaning of saved state.

Engine responsibilities:

- save slots;
- serialization;
- deserialization;
- file naming;
- metadata;
- timestamps;
- backups;
- corruption/error reporting;
- save version number;
- load pipeline;
- optional migration hooks.

Conceptual structure:

```text
SaveContainer
├── engineMetadata
├── saveVersion
├── timestamp
└── storyState
```

The Story state may contain any Story-defined keys/modules.

The save system must not require dedicated fields such as `money`, `relationship`, or `inventory` in Engine code.

---

### 5.14 Input

The Engine shall provide an input abstraction for common runtime actions.

Examples:

- confirm;
- cancel;
- advance dialogue;
- open menu;
- skip;
- auto mode;
- pointer/click input.

Story systems may define additional actions without modifying the Engine core.

---

### 5.15 Logging and Diagnostics

The Engine shall provide structured runtime logging.

Useful categories:

```text
[Engine]
[State]
[Command]
[Signal]
[Content]
[Yarn]
[Save]
[Asset]
[UI]
[Story]
```

Story module logs may be routed through the same logger.

The logger should make state transitions and command failures easy to inspect.

---

### 5.16 Debug Tools

A developer/debug panel is a foundation feature, not a late polish feature.

Minimum capabilities:

- inspect state keys;
- edit state values;
- inspect registered modules;
- inspect content IDs;
- inspect recent signals;
- execute commands;
- start a Yarn node;
- save/load;
- reload Story content where safe;
- show validation errors.

Story modules may contribute custom debug panels or actions.

---

### 5.17 Validation

The Engine shall validate content before runtime where practical.

Examples:

- duplicate content ID;
- missing asset ID;
- missing Yarn node;
- unknown command;
- invalid condition;
- missing module dependency;
- invalid Story manifest;
- invalid save schema/version.

A future Unity editor command should provide:

```text
Tools → Visual Game Engine → Validate Content
```

with errors and warnings.

---

## 6. Story Responsibilities

The Story package contains **everything that defines the actual game**.

### 6.1 Game Rules

Examples:

- time model;
- relationship formulas;
- economy;
- inventory rules;
- hunger/energy;
- activity costs;
- location access;
- shop logic;
- jobs;
- progression;
- route rules;
- unlock requirements.

If a rule could reasonably differ between two games, it belongs in Story.

---

### 6.2 Game Flow

Story defines:

- events;
- triggers;
- conditions;
- actions;
- priorities;
- progression;
- chapters;
- routes;
- activities;
- scene sequencing.

Example:

```text
event: alice_first_date

trigger:
  location_entered

conditions:
  alice.relationship >= 20
  world.day >= 5
  flags.alice_first_date == false

actions:
  start Yarn node AliceFirstDate
```

The Engine evaluates and executes generic primitives, but the event definition belongs entirely to Story.

---

### 6.3 Dialogue

All narrative dialogue and choices belong to Story Yarn files.

Suggested organization:

```text
Story/Yarn/
├── Main/
├── Characters/
├── Events/
├── Activities/
└── Phone/
```

---

### 6.4 Characters

Character identity and game meaning belong to Story.

Possible Story data:

- ID;
- display name;
- starting values;
- sprites;
- expressions;
- outfits;
- portraits;
- voice configuration;
- location rules;
- Story-specific stats.

The Engine should not have a concrete `Alice.cs`.

---

### 6.5 Locations

Story defines locations and navigation rules.

Examples:

- bedroom;
- kitchen;
- cafe;
- office.

Story decides:

- how locations connect;
- travel requirements;
- travel cost;
- travel time;
- available activities;
- active characters;
- background IDs.

Engine only provides generic view/content/presentation capabilities.

---

### 6.6 Assets

All game-specific assets belong to Story:

- character sprites;
- expressions;
- outfits;
- backgrounds;
- CG;
- UI art;
- icons;
- BGM;
- ambience;
- SFX;
- voice;
- fonts where licensing allows.

Assets should be referenced by stable IDs.

---

### 6.7 UI Content and Theme

Story owns:

- visual theme;
- HUD composition;
- phone design;
- shop screen;
- relationship screen;
- map layout;
- story-specific menus.

Engine owns only reusable UI hosting and technical primitives.

---

## 7. Recommended Repository Structure

Folder layout below is **authoring-time Unity project structure**, not shipping Story package contract. Runtime Story package content must be logically separate from compiled Engine, with manifest/catalogs defined in §8 and ARCHITECTURE.md. Avoid assuming arbitrary runtime C# assemblies are hot-loadable.

Initial target:

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

Assembly dependency:

```text
VisualGameEngine.Story
        │
        ▼
VisualGameEngine.Runtime
```

The reverse dependency is prohibited.

---

## 8. Story Package Contract

Story loads by manifest, not through Engine hardcoding. Proposed first fields: schemaVersion, id, version, requiredEngineApi, entrypoint, contentCatalogs, dialogueCatalogs, screenCatalogs, eventCatalogs, assetCatalogs, modules. Full sample and constraints: [ARCHITECTURE.md](./ARCHITECTURE.md#3-package-layout-proposed).

Minimum behavior:
- validate Story/Engine schema and API compatibility, stable IDs, unique references and entrypoint before gameplay;
- load one active Story and namespace identifiers to prevent silent collisions;
- treat Story data/UI/Yarn/assets as replaceable, with optional interpreted rule scripts behind an evaluated adapter;
- expose Story-specific commands through registration; keep gameplay meaning out of Engine;
- never assume native C# hot-reload works on production platforms;
- keep asset packaging tech a separate technical decision.

First authored sample may live in Unity project, but contract must not require modification of Engine assemblies. Do not promise runtime bundle swapping until proven.

## 9. Dependency Policy

Preferred dependencies:

- Unity built-in packages;
- Yarn Spinner;
- free/open-source libraries with compatible licenses.

Foundation code should avoid unnecessary framework dependencies.

A dependency must justify at least one of:

- substantial implementation time saved;
- proven stability;
- difficult technical problem solved;
- strong interoperability benefit.

Do not add a package merely because it may be useful later.

---

## 10. Foundation Milestones

[ROADMAP.md](./ROADMAP.md) defines current authoritative order:

0. Repo, pinned deps, Engine/Story assembly boundary, manifest contract.
1. Kernel, IDs, registration and Story loader.
2. Playable VN with declarative UI, Yarn and visual/audio primitives.
3. Safe-checkpoint persistence and generic Story flow/event execution.
4. Story-defined custom UI/rules; evaluate optional Lua adapter.
5. Second distinct Story, same Engine source/binary, integration tests.
6. Creator tools, validation, docs and release hardening.

Each milestone has runnable acceptance gates. No feature is complete solely because its interface has been documented.

## 11. Reference Story / Vertical Slice

The repository should contain a very small sample Story used only to validate the engine contract.

Example sample:

```text
Room A
Room B
Character A
one Yarn conversation
one choice
one Story-defined variable
one Story module
one conditional event
one image/CG
one audio track
save/load
```

The sample is not the actual game's architecture. It is a test client for the Engine.

The important test is that the sample can later be deleted and replaced without changing Engine code.

---

## 12. Engineering Principles

### Engine must be generic

Avoid:

```csharp
IncreaseAliceLove();
GivePlayerMoney();
GoToBedroom();
StartFirstDate();
```

Prefer:

```csharp
State.Set(...);
Commands.Execute(...);
Signals.Emit(...);
Views.Show(...);
```

Story modules compose those primitives into game behavior.

### Prefer composition over inheritance

Large inheritance trees should be avoided. Prefer small services/modules that compose behavior.

### Prefer stable IDs over file paths

Story code/data should refer to:

```text
character.alice.happy
```

rather than:

```text
Assets/Story/Characters/Alice/alice_happy_final_v4.png
```

### Avoid premature abstraction

The Engine should be generic, but not theoretical.

A generic system should be added because at least one real Story requirement needs it.

### Keep public APIs small

The Engine API exposed to Story should be intentionally small and stable.

Candidate top-level API:

```text
State
Commands
Signals
Modules
Content
Assets
Views
Audio
Dialogue
Save
UI
Input
```

---

## 13. Initial Public API Direction

Conceptual only; exact C# design will be decided during implementation.

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

Story modules may build higher-level APIs such as:

```text
Time.Advance(...)
Inventory.Add(...)
Location.Travel(...)
Relationship.Change(...)
Shop.Buy(...)
```

Those APIs remain outside Engine.

---

## 14. Key Architectural Decision

When deciding whether a feature belongs in Engine or Story, ask:

> **Could a completely different game reasonably not need this feature or need different rules for it?**

If **yes**, it normally belongs in Story.

Examples:

| Capability | Owner |
|---|---|
| State storage | Engine |
| Command execution | Engine |
| Signal bus | Engine |
| Asset loading | Engine |
| Generic UI hosting | Engine |
| Audio playback | Engine |
| Save-file infrastructure | Engine |
| Yarn integration | Engine |
| Relationship rules | Story |
| Time rules | Story |
| Inventory rules | Story |
| Economy | Story |
| Characters | Story |
| Locations | Story |
| Story events | Story |
| Dating | Story |
| Jobs | Story |
| Quests | Story |
| Phone | Story |

---

## 15. Open Decisions

Resolve via prototypes and record decisions, rather than guessing package or library behavior:

1. Exact Unity 6 LTS, Yarn and third-party dependency versions/licenses/target platforms.
2. Authoring-to-runtime package pipeline: serialized Unity assets, catalog and bundles/Addressables.
3. Declarative UI schema and binding/update semantics.
4. Canonical Engine state store ↔ Yarn variable types and lifecycle.
5. Save safe-point and Yarn continuation support; future rollback semantics.
6. Optional interpreted scripting: language/runtime, sandbox limits, IL2CPP, profiling.
7. Story version migration compatibility, dev reload and native plugin packaging.

See [ARCHITECTURE.md](./ARCHITECTURE.md#9-deferred-design-decisions).

## 16. Definition of Foundation Complete

The first engine foundation is complete when two meaningfully different replacement Story packages run with the same Engine revision and a replacement Story package can:

1. register its own modules;
2. define arbitrary state keys;
3. register commands;
4. emit/listen to signals;
5. resolve and present its assets;
6. run Yarn dialogue;
7. call Story-specific behavior from Yarn;
8. evaluate Story-defined conditions;
9. save/load Story state;
10. use debug/validation tools;

without modifying the Engine assembly.

At that point the project has a real reusable engine/story boundary and can safely begin building the actual game's systems on the Story side.
