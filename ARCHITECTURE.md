# Architecture Proposal — Engine + Dynamic Story Packages

**Status:** Proposed v0.2, docs-only. **Goal:** Ren'Py-like authoring flexibility on Unity, with replaceable Story content and stable Engine runtime.

## 1. Product boundary

**Engine** is compiled, reusable technology; not a hard-coded game. **Story** is a package of configurable/supported gameplay, narrative, UI, data, and assets. Engine is upgradable/extensible but stays independent of any Story.

"Story can change anything" means **anything representable through Engine's public APIs and supported scripting/UI primitives**. New native Unity components, shaders/pipelines, platform integrations, or unexposed capabilities require a compiled adapter/plugin or Engine update. Do not promise arbitrary C# hot loading or unrestricted modification of Unity internals.

Three extension tiers:
1. **Content/config:** change dialogue, assets, localization, data, events, screens with no Engine changes.
2. **Story behavior:** define conditions, commands, rules via supported declarative actions and optional interpreted scripts; no Engine rebuild.
3. **Native extensions:** C# adapters, custom renderer/widgets, platform integrations; ship or rebuild as code, not ordinary Story assets.

## 2. Dependency direction

~~~text
  Authoring tools / editor / validator
                  |
           Story package(s)
     manifest, data, Yarn, UI, assets,
      events, optional story scripts
                  |
                  v
          Engine public API
  state | commands | signals | flow |
  content | assets | views | UI | audio |
  input | dialogue | save | diagnostics
                  |
        Engine adapter modules
           Unity / Yarn / Script*
                  |
                Unity
~~~

*Script adapter is candidate, not approved dependency.*

Rules:
- Engine core/runtime assemblies NEVER reference Story assemblies, namespace, IDs, gameplay concepts, or assets.
- Story may reference only documented Engine-facing contracts. Generic Engine services do not call concrete Story types.
- Yarn adapter depends on Yarn and Engine contracts; Engine core does not depend on Yarn.
- Engine modules/Story modules register via interfaces, not hard-coded source references. Native Story module C# code is an extension requiring build/deployment, not a dynamically loaded content script.
- Story editing must not require editing Engine C# to add currencies, relationship rules, jobs, schedules, UI screens, or events, provided existing API capabilities suffice.

## 3. Package layout (proposed)

~~~text
StoryPackages/
  demo-novel/
    manifest.json
    config/
    data/
    dialogue/          # Yarn source in dev; compiled assets in release
    events/
    scripts/           # optional, only if script runtime approved
    screens/
    themes/
    localization/
    assets/
~~~

Authoring layout may use Unity ScriptableObjects. Shipping format should be **logical package + manifest**; exact bundle/container tech is pending. JSON is appropriate for non-Unity config; Unity assets use Engine resolver and may use direct refs in editor/build or Addressables later. File paths are internal implementation details; Story APIs use stable content IDs.

### Manifest contract candidate

~~~json
{
  "schemaVersion": 1,
  "id": "demo.novel",
  "version": "0.1.0",
  "requiredEngineApi": "0.1",
  "entrypoint": "intro",
  "contentCatalogs": ["data/catalog.json"],
  "dialogueCatalogs": ["dialogue/catalog.json"],
  "screenCatalogs": ["screens/catalog.json"],
  "eventCatalogs": ["events/catalog.json"],
  "assetCatalogs": ["assets/catalog.json"],
  "modules": []
}
~~~

Fields and contract are **proposed**, not existing executable implementation. Default runtime loads one active Story at a time; overlapping IDs across packages must never silently overwrite. Future dependencies/multiple active packages require explicit namespace and conflict rules. Story ID must be durable across versions. Manifest must carry schema and Engine API compatibility. Startup rejects incompatible schema/API, duplicate IDs, missing dependencies/assets and invalid entrypoint with actionable diagnostics.

Package load stages:
1. Read/validate manifest, compat and source safety (normalized paths, size limits, integrity where feasible).
2. Register modules, content IDs, catalogs, variable defaults and optional scripting functions.
3. Resolve asset references and compile/resolve Yarn nodes and UI/event defs; aggregate errors.
4. Initialize Engine state, module lifecycle, adapters, UI host and Story entrypoint.
5. Activate gameplay only when required content validates. On failure, keep prior valid state or return explicit error.

Development hot-reload is not equivalent to live package swapping. Start with explicit restart/reload of package in dev; preserve runtime state only if reload contract can guarantee safety.

## 4. Runtime pipeline

~~~text
Player input
   -> Story-declared UI action / Yarn choice
   -> Engine command dispatcher (typed args, validation)
   -> Story rule / generic Engine primitive
   -> State update + signal
   -> Story event condition + deterministic action sequence
   -> Yarn node / View / UI / Audio through adapters
~~~

- **State:** typed shared source of truth. Gameplay variables and Yarn variables must have explicit mapping, type conversion and namespaced keys; no unsynchronized parallel stores. Choose per-key persistence policy (persistent/session/temporary) before shipping saves.
- **Commands:** registered stable IDs, typed inputs, async completion/cancel semantics, scoped errors and logging. Engine implements technical commands; Story implements game rules.
- **Signals:** subscribe/unsubscribe with explicit lifecycle, ordering guarantee within one dispatch, reentrancy guard/loop protection; Story owns triggers.
- **Flow/events:** Story declares conditions, triggers, priority and action sequence; Engine evaluates primitives and schedules actions. Define deterministic dispatch and same-frame recursion rules before shipping.
- **Dialogue:** Yarn adapter owns presentation bridge and node start/stop; Story owns Yarn content, choice consequences and any custom commands. Avoid reimplementing Yarn syntax in Engine.
- **Visual/UI:** Engine owns generic view layers, animation and UI widget host. Story defines declarative widget trees, bindings, actions, themes, layout, screens and menus. New widget types are native extensions.
- **Assets:** stable IDs to assets via catalog/resolver, async load, cache/release. No direct Story-facing Unity file paths.

### Declarative screen proposal

~~~json
{
  "id": "ui.main_menu",
  "type": "screen",
  "children": [
    {
      "type": "button",
      "text": "Start",
      "onClick": { "command": "dialogue.start", "args": { "node": "intro" } }
    }
  ]
}
~~~

Keep supported widget/action vocabulary small: screen, panel, text, image, button, list, overlay, binding, open/close, emit command. Schema validation must catch unknown widgets/commands and missing IDs. Story-specific C# MonoBehaviours are NOT required for ordinary screen changes.

## 5. Scripting decision

**MVP:** Yarn for dialogue + declarative conditions, event/actions and typed commands. **Candidate future tier:** Lua hosted behind an optional Engine scripting adapter (evaluate MoonSharp or another compatible maintained interpreter, licensing, Unity player targets, IL2CPP behavior, debugging and performance before acceptance).

Do not adopt Lua merely for flexibility:
- Define exact Story API allowlist (state, commands, signals, content, UI). No unrestricted Unity/C# reflection/file/network interfaces exposed to Story.
- Define timeouts/cancellation, error reporting, deterministic sequencing, data exchange and save-safe points.
- Script execution safety must be proven; sandbox claims need adversarial tests. Untrusted third-party scripts not allowed by default until threat model + confinement validated.
- Avoid duplicate orchestration logic: Yarn = dialogue; events = triggers; optional scripts = complex rules/predicates, not competing full gameplay scheduler.
- Support Story-defined logic within Engine API. Native plugin code changes remain build-time.

## 6. Save/load contract

Engine handles atomic save writes, backups, versioning, validation and settings. Story owns runtime meaning, schema defaults and migrations.

~~~json
{
  "formatVersion": 1,
  "engineApi": "0.1",
  "storyId": "demo.novel",
  "storyVersion": "0.1.0",
  "state": {},
  "modules": {},
  "checkpoint": {}
}
~~~

First milestone supports explicit **safe checkpoints** (idle/command boundary) and restores reproducible Story state and view. Do not promise exact restoration of in-flight Yarn execution, async commands, animation clocks or rollback unless adapters expose serializable continuation. Restoring unsupported in-flight execution must fail clearly or intentionally restart from recorded safe point. Story version mismatches require migration hook or explicit refusal; avoid silent corruption.

## 7. Packaging / versioning / security

- Separate editor assets vs portable runtime manifest/content. Ship only supported formats on target platform; Unity asset delivery may require build-target-specific bundles.
- Keep API version distinct from Story content version and save schema version.
- Validate manifest IDs, internal paths (block traversal), duplicates, types, missing refs, command argument shapes, Yarn nodes, UI bindings, incompatible versions and unsupported script privileges.
- Clarify trusted authored content vs user-installed mods. Asset IDs are not a security boundary.
- Pin exact Unity LTS editor, Yarn package and any additional deps in Phase 0; record licenses and compatibility. No version guesses.
- Profile mobile/desktop target performance before selecting scripting/asset tech.

## 8. Release gates

**Gate A (minimum VN):** One Story with intro, dialogue choice, background/character/CG, declarative menu, save/relaunch; no game-specific Engine code.

**Gate B (different genre):** Second independent life-sim-like Story with custom UI, events, navigation, arbitrary values/rules. Use exactly same Engine revision/binary as Gate A. Engine source unchanged.

**Gate C (replacement):** Remove first Story, run second, confirm Engine assembly compiles without any Story references, and all failures from missing/duplicate content surface in validator.

**Gate D (persistence):** Save/restart/restore at safe points, incompatible versions rejected or explicitly migrated; no partial corrupted load.

**Gate E (extensions):** Demonstrate Story-defined command/rule registration without touching Engine; demonstrate one native plugin extension as separate optional assembly later.

## 9. Deferred design decisions

Resolve through runnable spikes, then write ADRs:
- Story runtime packaging: Unity asset catalog vs Addressables vs build-time bundles; target platforms.
- Interpreter choice (Lua or none) and security/perf evaluation.
- UI schema shape, widget vocabulary and binding update model.
- Yarn variable mapping, types, save/continuation guarantees.
- Asset and Story migration lifecycle; script integration API.
- Exact Unity 6/Yarn versions, target player/IL2CPP support, dependency licenses.

**Non-goals v0.2:** multiplayer, arbitrary native DLL hot-loading, replacing Unity, copying Ren'Py Python/screen grammar, full visual editor, general RPG logic in Engine, uncontrolled live reload, rollback across arbitrary async tasks.
