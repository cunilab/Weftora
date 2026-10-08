---
id: WFT-PRD-001
title: Weftora Product Requirements
type: product_requirements
doc_version: 0.4.1
status: proposed
implementation: not_started
created: 2026-09-29
updated: 2026-10-08
last_reviewed: null
owner: Weftora Maintainers
scope: windows-first
product_release: unreleased
related_pr: 1
supersedes: null
---

# Product Requirements — Weftora

**Repo:** `cunilab/Weftora`  
**Architecture:** [docs/architecture.md](./docs/architecture.md)  
**Roadmap:** [ROADMAP.md](./ROADMAP.md)  
**Platforms:** [docs/platforms.md](./docs/platforms.md)  
**Phase gates:** [docs/success-criteria.md](./docs/success-criteria.md)

## 1. Product vision

Build **Weftora, a standalone Rust-native visual game engine with its own player and creator tools** that makes narrative and 2D story-heavy games quick to author and change. Inspiration: Ren'Py's end-to-end storytelling workflow and Yarn's approachable dialogue scripting. This project is an **alternative engine**, not a Unity extension, Yarn renderer, or Rust/Bevy framework developers must embed in their own game code.

Contract: **Engine ships as compiled game player + creator tools; Story package supplies gameplay behavior, rules, flow, UI definitions, dialogue, state schema, content and assets.** Most changes in a game must not require Engine rebuild. Native capabilities require explicit Engine/backend extensions.

## 2. Users and use cases

- **Author:** install prebuilt Weftora tools; write Yarn dialogue, describe scenes, actions, rules and screens as data; preview without Rust/Cargo/Bevy project; get actionable errors.
- **Game developer:** compose Story-specific rules, events and screens through content/scripts using supported Engine APIs; Rust only for native Engine extensions.
- **Player:** load a packaged game and experience reliable choices, visuals/audio and save/load.
- **Engine maintainer/AI agent:** change core safely under mechanical architecture and regression gates.

**MVP platform: Windows x64 (`x86_64-pc-windows-msvc`) only.** Fully validate narrative engine, two Stories, creator workflow, save/load and Windows export before porting to Linux, macOS, Android, iOS or Web. Design platform boundaries early but **defer other platform implementation/build/test pipelines** until Windows foundation release gate. No broad 3D/RPG ambition in foundation.

## 3. Goals / success criteria

1. **Standalone reusable engine:** one compiled Weftora player/revision runs two distinct Story packages without Engine source changes and without custom Rust game binaries.
2. **Replaceable Story:** author can change data, dialogue, event rules, themes, screens and assets, then validate/run using prebuilt Engine without Rust compiler/Cargo or recompiling player.
3. **Renderer independence:** headless API/state/flow/save crates do not depend on Bevy, Yarn or example Story.
4. **Simple content authoring:** text-based content formats, stable IDs, schema-backed diagnostics, CLI preview and tooling.
5. **Reliable narratives:** Yarn choices + Story commands + generic flow/events + coordinated single gameplay state.
6. **Persistence:** safe-checkpoint save/restore, version checks, corruption handling.
7. **Maintainable Rust:** typed public contracts, bounded extension points, dependency guardrails, CI and reference Story tests.
8. **Free/open tooling:** choose transparent, compatible dependency licenses and pin tested versions.
9. **Portable architecture, focused execution:** core/story package interfaces avoid Windows-specific assumptions, but only Windows x64 is required or tested before platform expansion.

## 4. Non-goals for foundation

- Unity integration, engine-specific Unity assets or MonoBehaviours.
- Full-featured 3D world/editor, physics, multiplayer or networked game simulation.
- Built-in dating/economy/relationship/quest/phone/inventory mechanics in Rust core.
- Custom competitor to Yarn language/parser before evidence demands it.
- Arbitrary Rust/C++ native dynamic code loading through Story packages.
- Guaranteed sandbox for untrusted mods before explicit threat model/test gate.
- General arbitrary-state rollback or serialization of in-flight tasks/animations.
- Full GUI editor before headless runtime + playable slice exist.

## 5. Core functional requirements

### R1. Package discovery, integrity and compatibility
- Load external Story manifest, content catalogs and versioned entrypoint through generic loader.
- Validate namespaced IDs, schema/API range, required features, catalog references and file paths.
- Reject invalid packages atomically, report file/location and cause; avoid partial active Story state.
- Support one active Story at MVP; future dependencies/mods require explicit compatibility model.

### R2. Lifecycle and runtime boundaries
- Provide boot/load/ready/play/pause/shutdown lifecycle and ordered init/teardown with errors.
- Engine API types independent of rendering and narrative backends; compile `weftora-core` without Bevy/Yarn.
- No Story-specific names, assets, types or gameplay assumptions in Engine core.

### R3. Typed state
- Namespaced keys; bool/integer/finite float/string values initially; get/set/add/remove, defaults, change notifications.
- Explicit state scopes and save policies; deterministic serialization and clear type errors.
- Yarn variable bridge must share canonical Engine state or implement tested synchronization.

### R4. Command + signal framework
- Typed, validated commands with stable IDs, result/errors, async sequencing and cancellation.
- Deterministic within-dispatch signal ordering, subscribe/unsubscribe and loop protection.
- Story may register its own behavior from declarative actions and (future) opt-in script functions.

### R5. Generic conditions + flow
- State comparisons, boolean composition, conditions, triggers, priorities, ordered actions.
- Generic flow scheduler with async handling, traceability and bounded reentrancy.
- All game-specific event definitions authored in Story, not built into Rust runtime.

### R6. Dialogue adapter
- Start/stop Yarn nodes, show lines/choices, dispatch Story commands and expose mapped variables.
- Compiler and runtime errors identify Story file/node; retain backend replaceability.
- Rust Yarn port is work-in-progress; gate adoption on runnable compatibility tests.

### R7. Generic visuals + audio
- Render Story-defined backgrounds, multiple sprites, CG, layers, positioning, fades and transitions.
- Generic audio channels (BGM/SFX/voice), playback/fades/user volumes.
- Implementation via Bevy adapter initially, without leaking Bevy types into headless API.

### R8. Declarative UI
- Story defines screens, widgets, layout/theme, bindings, focus/input and actions using validated schema.
- Start with menu, dialogue line, choices, panel, image, buttons, overlays; grow from real use cases.
- Arbitrary new native widget types require adapter changes, not silent Story code injection.

### R9. Asset loading
- Stable Story asset IDs, catalog-backed resolution, caching/release, clear missing/duplicate errors.
- Image/audio/font formats and packaging pinned to explicit supported targets; not direct arbitrary file paths in public Story API.
- Development Story folder may differ from release Story bundle.

### R10. Save/load
- Versioned save container with Story identity, scoped state, module data and safe checkpoint.
- Atomic writes/recovery, save slot metadata, clear incompatible/corrupt state behavior.
- Version migration hooks defined; arbitrary mid-command/animation continuation out of scope until proven.

### R11. Story customization
- Define gameplay rules, characters, locations, events, conditions, UI behavior without modifying Rust Engine.
- Start with declarative flow and custom commands; evaluate optional Rhai adapter for complex rules.
- Script host, if approved, exposes allowlisted Engine services with bounded resources and error reporting.

### R12. Creator experience
- Standalone `weftora` CLI (implemented by `weftora-cli`) scaffolds, validates, previews and exports supported Story packages.
- Install prebuilt tools → author Story → run game → export standalone distribution; Story author does not write or compile Rust.
- Source-located errors for missing IDs, invalid actions, broken Yarn, incompatible manifests and unsupported widgets.
- Development reload/preview initially restart-based; true hot reload later when state rules permit.

### R13. Engine distribution vs framework dependency
- `weftora-player` standalone executable loads external Story folder/package selected at launch; no game-specific compiled Rust needed.
- `weftora export` bundles target-specific prebuilt player with validated Story content into playable distribution on supported platforms. Player binaries are built by Engine maintainers; cross-platform binaries not assumed available until tested.
- Story source is never a Cargo crate or mandatory Rust/Bevy project. Editor optional; CLI/content source first-class.
- Same compiled player runs Story A and Story B; change Story files without player rebuild. Native extensions require explicit Engine capability/release.

### R14. Tests and architecture enforcement
- CI fmt/clippy/tests and dependency-direction check; `#![forbid(unsafe_code)]` for headless crates where appropriate.
- Golden headless narrative/flow tests, manifest/schema fixture tests, safe save roundtrip and compatibility negatives.
- Two independent sample Story packages must pass on same built Engine/player.
- Rust compile success alone never substitutes for authoring UX or behavioral verification.

### R15. Windows-first shipping, future portability
- First supported build and authoring CLI: Windows x64 / `x86_64-pc-windows-msvc`. Windows release baseline includes game player, external Story loading, CLI `new/check/run/export`, audio, keyboard/mouse, scalable windowed UI, persistence at user-writable path and standalone export.
- Test Windows native filesystem (asset paths, Unicode, separators, permissions), graphics backend, app/window lifecycle, save directory, relative packaged paths, input focus, and clean launch on a Windows machine without developer toolchain.
- Platform-neutral core contracts: assets/storage, paths, time/lifecycle, input, renderer/audio, packaging; Windows implementations live behind platform adapters. Do not force Windows `std::path` semantics or absolute host paths into Story package schemas.
- No Linux/macOS/Android/iOS/Web implementation, CI matrix or release promise until Windows foundation exit gate. Windows portability architecture checks are allowed; unneeded cross-platform infrastructure is deferred.
- After gate: add one target platform at a time with target-specific packaging/signing/input/storage/renderer tests. See [platforms](./docs/platforms.md).

## 6. Ownership rules

**Engine:** lifecycle, state storage, generic command/signal/flow execution, rendering primitives, audio/input, view/UI host, package loading, persistence technology, diagnostics.

**Story:** variables/meaning, progression, rules, dialogue, events, locations, relationships, activities, shops, theme/UI definitions, localization, data/assets.

**Adapter/native extension:** rendering backend, dialogue interpreter, optional script host, new native widgets/platform services; explicit versioned Engine capabilities.

When unclear: if gameplay behavior changes between game genres, default to Story. If generic technical primitive is missing, update Engine API deliberately with tests and versioning.

## 7. Acceptance / release definition

- **Story A:** playable via stand-alone Weftora player: VN: dialogue + branches, choice-driven state, background/character/CG, audio, menu, safe save/relaunch.
- **Story B:** distinct life-sim-like rules, events, navigation and custom screens. Same **player binary** runs A and B without new Rust game crate or Engine edits.
- **Isolation:** deleting all Story packages still leaves Engine workspace compiling/tests passing; core's dependency graph contains no Story/Bevy/Yarn.
- **Authoring:** install prebuilt tools, modify Story behavior/UI/assets, validate/run/export without writing Rust, setting up a Bevy app or rebuilding compiled Engine.
- **Failure:** missing asset/node, duplicate IDs, invalid version/schema, unsupported UI and corrupt save give actionable errors; no silent partial load.
- **Quality:** Windows x64 release build, Story A/B regression suite, package validator, CLI/export install/run smoke test, audio/input/view, clean save/load/corruption recovery all pass; no unverified guarantee of scripting sandbox or rollback.

## 8. Architecture choices pending

- Initial target: Windows x64 only; exact Windows/graphics/runtime prerequisites confirmed by tests. Ports to Linux, macOS, Android, iOS and Web require Windows foundation exit gate + per-platform feasibility/release checks.
- Tested Rust/Bevy/Yarn compatible versions, licenses and support risk.
- Declarative UI schema and renderer choice.
- Rules language (declarative-only MVP; Rhai prototype optional).
- Story packaging and native resource processing.
- Yarn state bridge and safe checkpoint/continuation behavior.
- Stable API/Story/save versioning and migration policy.

Use [architecture](./docs/architecture.md) for rationale, [roadmap](./ROADMAP.md) for ordered execution, and [success criteria](./docs/success-criteria.md) for mandatory executable PASS gates. Claims here are requirements, not implemented features.

## Change History

| Date | Version | Change | Reference |
| --- | --- | --- | --- |
| 2026-10-08 | 0.4.1 | Standardize metadata/header and doc lifecycle. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
