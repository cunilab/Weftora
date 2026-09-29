# Roadmap

This roadmap covers the reusable **Engine foundation** first. Game-specific systems stay in the Story layer.

## Phase 0 — Repository Foundation

Goal: establish the Unity project and hard Engine/Story boundary.

- [ ] Create Unity 6 LTS project
- [ ] Add Yarn Spinner
- [ ] Create `VisualGameEngine.Runtime.asmdef`
- [ ] Create `VisualGameEngine.Story.asmdef`
- [ ] Enforce Story → Engine dependency only
- [ ] Create initial folder structure
- [ ] Add Git ignore rules
- [ ] Configure Git LFS for large assets if needed
- [ ] Add basic coding conventions

**Done when:** both assemblies compile and Story can reference Engine while Engine has no Story dependency.

---

## Phase 1 — Kernel

Goal: create the smallest reusable runtime core.

### Lifecycle

- [ ] Game bootstrap
- [ ] Initialization order
- [ ] Shutdown lifecycle
- [ ] Service registration

### State

- [ ] Generic typed state store
- [ ] `Get`
- [ ] `Set`
- [ ] `Add`
- [ ] `Remove`
- [ ] State-change notifications

### Communication

- [ ] Command registry
- [ ] Command execution
- [ ] Async command support
- [ ] Signal/message bus
- [ ] Subscribe/unsubscribe
- [ ] Signal payload support

### Modules

- [ ] `IGameModule`
- [ ] Module registration
- [ ] Module initialization
- [ ] Module shutdown
- [ ] Dependency/error reporting

### Diagnostics

- [ ] Structured logger
- [ ] Engine/Story log categories

**Done when:** a Story module can initialize, change state, register a command, and communicate through signals without custom Engine changes.

---

## Phase 2 — Content and Assets

Goal: Story references stable IDs instead of hard-coded files.

- [ ] Content registry
- [ ] Stable content IDs
- [ ] Duplicate-ID detection
- [ ] Missing-ID errors
- [ ] Asset resolver
- [ ] Sprite/image loading
- [ ] Audio loading
- [ ] Prefab/generic Unity object loading
- [ ] Basic cache
- [ ] Asset unloading strategy

Initial implementation can use direct Unity references or ScriptableObjects. Addressables can be added later behind the same API.

**Done when:** Story can request an asset by ID and Engine resolves and loads it.

---

## Phase 3 — Presentation

Goal: expose reusable visual and audio primitives.

### Views

- [ ] Generic view host
- [ ] Show/hide
- [ ] Position
- [ ] Scale
- [ ] Layer/order
- [ ] Opacity
- [ ] Fade
- [ ] Move
- [ ] Crossfade
- [ ] Basic shake/effects

### UI

- [ ] Screen host
- [ ] Panel host
- [ ] Modal/overlay host
- [ ] Popup/notification support
- [ ] Input blocking during transitions

### Audio

- [ ] BGM channel
- [ ] SFX channel
- [ ] Ambience channel
- [ ] Voice channel
- [ ] Volume settings
- [ ] Fade/crossfade

**Done when:** Story can present backgrounds, characters, CGs, UI, and audio using only generic Engine APIs.

---

## Phase 4 — Yarn Integration

Goal: make Yarn the default Story dialogue layer without putting game rules into Engine.

- [ ] Yarn adapter
- [ ] Start node API
- [ ] Dialogue lifecycle signals
- [ ] Read Engine state from Yarn
- [ ] Write Engine state from Yarn
- [ ] Expose generic Engine commands
- [ ] Allow Story modules to register Yarn commands
- [ ] Allow Story modules to register Yarn functions
- [ ] Clear Yarn error reporting

**Done when:** a Story Yarn file can run dialogue, access state, and invoke Story-defined behavior.

---

## Phase 5 — Conditions and Flow Primitives

Goal: provide generic tools for Story to build its own event/flow systems.

- [ ] Condition interface/model
- [ ] Equality operators
- [ ] Numeric comparisons
- [ ] AND
- [ ] OR
- [ ] NOT
- [ ] State-value conditions
- [ ] Story-registered custom predicates
- [ ] Generic action sequence execution
- [ ] Async action sequences

Do **not** build dating, quests, time, locations, or inventory into Engine.

A reference Story-side event module may be created to prove the API.

**Done when:** Story can define a conditional flow/event without adding game-specific logic to Engine.

---

## Phase 6 — Persistence

Goal: save any Story-defined runtime state without Engine understanding its meaning.

- [ ] Save container format
- [ ] Save slots
- [ ] State serialization
- [ ] Module save hooks
- [ ] Module load hooks
- [ ] Save metadata
- [ ] Timestamp
- [ ] Save version
- [ ] Backup/recovery handling
- [ ] Migration hook
- [ ] User settings separated from game progress

**Done when:** quit/relaunch restores the same Story state and Story modules can persist their own data.

---

## Phase 7 — Debugging and Validation

Goal: make content-heavy development fast to test.

### Runtime Debug

- [ ] State inspector
- [ ] Edit state values
- [ ] Module inspector
- [ ] Command runner
- [ ] Signal history
- [ ] Content registry inspector
- [ ] Start Yarn node manually
- [ ] Save/load controls

### Validation

- [ ] Duplicate content IDs
- [ ] Missing asset IDs
- [ ] Invalid conditions
- [ ] Unknown commands
- [ ] Missing Yarn references
- [ ] Module dependency errors
- [ ] Story manifest validation

Target editor entry:

```text
Tools → Visual Game Engine → Validate Content
```

**Done when:** common Story mistakes can be identified without debugging C# manually.

---

## Phase 8 — Reference Story

Goal: prove that the Engine is reusable.

Create a tiny sample Story containing:

- [ ] two simple locations
- [ ] one character
- [ ] one Yarn conversation
- [ ] one choice
- [ ] arbitrary Story state
- [ ] one Story module
- [ ] one conditional event
- [ ] one background
- [ ] one character image
- [ ] one CG
- [ ] one audio track
- [ ] save/load

The sample Story is a test client, not part of Engine architecture.

**Done when:** the entire sample can be deleted and replaced by another Story without modifying Engine code.

---

## Phase 9 — Foundation Release

Goal: stabilize the reusable API.

- [ ] Review public Engine API
- [ ] Remove Story-specific assumptions
- [ ] Add API documentation
- [ ] Add setup guide
- [ ] Add example Story documentation
- [ ] Add automated tests for core systems
- [ ] Confirm supported Unity version
- [ ] Confirm Yarn Spinner version
- [ ] Choose project license
- [ ] Tag first foundation release

## Foundation Exit Criteria

The Engine foundation is ready when a Story package can:

1. register modules;
2. define arbitrary state;
3. register commands;
4. emit and receive signals;
5. register and resolve content;
6. load and present assets;
7. run Yarn dialogue;
8. register Story-specific Yarn behavior;
9. evaluate conditions;
10. save/load its own state;
11. use debug and validation tools;

without modifying Engine source code.
