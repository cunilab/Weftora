# Roadmap — Reusable Engine + Dynamic Story

**Status:** v0.2 proposal. Each phase produces a runnable/testable contract. Story rules/UI/data remain replaceable; Engine remains generic.

## Phase 0 — Repo + contracts

- [ ] Create Unity 6 LTS project, pin exact editor and package versions.
- [ ] Install pinned Yarn Spinner; create Engine.Core, Unity runtime and optional Yarn adapter assemblies.
- [ ] Story assembly references Engine API; Engine assemblies never reference Story.
- [ ] Define Story manifest schema v1, stable ID rules, API compatibility policy, package loading errors.
- [ ] Choose repo license and dependency license inventory; Git ignore/LFS, CI and test assemblies.
- [ ] Add minimal Story example and architecture decision records.

**Gate:** clean compile, automated dependency-direction check, Engine compiles after deleting Story, invalid manifest rejected.

## Phase 1 — Kernel + loading

- [ ] Bootstrap/service lifecycle; module registry/dependency order.
- [ ] Typed state store + namespaced keys + change events.
- [ ] Command registry with typed validation, result/error, async cancellation.
- [ ] Signal bus with cleanup, ordering and loop protection.
- [ ] Content ID/catalog registry + duplicate/missing ID validation.
- [ ] Load single Story manifest and launch entrypoint; diagnostics.

**Gate:** load Story A, register Story command/module, mutate state, emit signal, reject malformed package.

## Phase 2 — First playable VN

- [ ] Asset resolver (direct refs or ScriptableObjects first; stable ID API).
- [ ] Generic layers: background, characters, CG, UI; show/hide/position/fade.
- [ ] UI host with small declarative widget/action schema; basic input.
- [ ] Audio channels BGM/SFX/voice.
- [ ] Yarn adapter, node/choice runner, command bridge, single mapped state source.
- [ ] Sample Story A: intro, branching conversation, choice, CG and menu.

**Gate:** fully playable VN slice, only Story-defined content/rules, Engine unchanged.

## Phase 3 — Save/load + flow

- [ ] Atomic versioned save container, Story identity/version, schema checks.
- [ ] Safe checkpoint contract; reconstruct UI/visual state at safe points.
- [ ] Module persistence hooks/migrations; corruption and mismatch errors.
- [ ] Declarative condition evaluator, event trigger registry, priority + action sequences.
- [ ] Explicit signal/event execution ordering and failure handling.
- [ ] One Story A conditional event and save/relaunch test.

**Gate:** deterministic Story state restore; no claim of mid-command/animation rollback.

## Phase 4 — Dynamic rules + custom screens

- [ ] Story-defined screens, layouts, bindings, themes, navigation and validation.
- [ ] Story-defined custom commands/functions through public Engine API.
- [ ] Prototype Lua interpreter against target Unity/IL2CPP, compatibility and security.
- [ ] If prototype passes: optional script adapter, allowlisted API, errors, execution budget, script/state binding.
- [ ] If prototype fails: retain declarative commands/conditions and document missing capabilities.
- [ ] Editor command to validate Story catalogs, Yarn references, UI IDs/commands.

**Gate:** game-specific rules/screens change with Story only; no C# Engine edits.

## Phase 5 — Second independent Story

- [ ] Story B with custom life-sim-like UI, locations, schedules, events, economy variables.
- [ ] Swap Story A → B without rebuilding/changing Engine code (within supported content/script pipeline).
- [ ] Assert Engine source/assemblies contain no Story IDs and Story A deletion does not break Engine compile.
- [ ] Reject duplicate IDs, invalid API versions, missing content and bad save migration.
- [ ] Integration/playmode tests across both Story packages.

**Gate:** same Engine revision/binary runs two meaningfully different games.

## Phase 6 — Creator workflow + release hardening

- [ ] Story package validator + diagnostic report and fast edit-preview-reload cycle.
- [ ] Basic authoring docs, sample projects, API docs and debug/state/event inspector.
- [ ] Test target platforms/IL2CPP, asset import/load/cache and memory behavior.
- [ ] Error recovery, save backups, licensing review, package compatibility tests and CI.
- [ ] Decide later: Addressables, editor extensions, localization pipeline, partial hot reload, rollback.
- [ ] Tag foundation release only after gates pass.

**Gate:** another creator can author/swap Story with docs and no Engine-source changes.

## Sequence guardrails

Do not build editor, complex Lua scripting, arbitrary mods, Addressables or life-sim-specific Engine systems before proving VN vertical slice and package contract. Fix missing Engine public primitives through explicit versioned changes; do not sneak Story gameplay into Engine.
