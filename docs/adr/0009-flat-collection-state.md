---
id: WFT-ADR-009
title: ADR 0009 — Flat Collection State
status: proposed
---

# ADR 0009 — Flat Collection State

## Decision

Add `list` and `map` to canonical state under the additive `state.collections/1` capability, fulfilling the first collection slice of [ADR 0003](./0003-story-rules-language.md). Lists preserve element order; maps use string keys and deterministic UTF-8 key order. Both contain scalar bool/int/finite-float/string values, including mixed types. Empty collections are valid. Nested collections and null elements/entries are rejected. A declared root kind stays fixed; there is no element schema.

Manifest defaults and `initialState` use ordinary JSON arrays/objects. A collection-bearing declaration or initial value requires the capability. Every rule action target must already be declared, so collection writes inherit that capability gate. The capability advertises support; older players refuse these packages before activation. Core writes validate Rust-constructed values too; rejected writes do not alter values, defaults or notifications.

Collections have at most 256 entries and 16384 payload bytes. Each entry costs eight bytes, plus its UTF-8 string bytes and (for maps) UTF-8 key bytes. Map keys are arbitrary strings, including empty strings, capped at 512 bytes; they do not become canonical state paths. Child strings are capped at 16384 bytes and must also fit the aggregate payload limit. JSON decoding rejects nesting, duplicate map keys and excessive collection growth. Tagged action deserialization may buffer input first; the existing source-size, duplicate-key and JSON-depth checks still apply.

## Runtime and authoring

Existing `state.set`, `state.reset` and `state.remove` operate on whole collections. Numeric `state.add`, conditions, command parameters and signal payloads remain scalar-only. Bounded local entry edits through Rhai are defined in [ADR 0010](./0010-bounded-rhai-host.md); read-only Yarn scalar queries are defined in [ADR 0012](./0012-yarn-collection-queries.md). Authors may maintain explicit scalar views alongside collection writes; Yarn variables and UI text bindings reject collection values rather than stringify or round them.

In rule actions and call arguments, `{"literal": value}` explicitly wraps a collection. This preserves the existing `{"argument":"name"}` syntax even for maps with `argument` or `literal` keys. Wrapping a scalar is also accepted. Bare arrays/objects in action value slots refuse with a format diagnostic. Manifest defaults and screen `state.set` actions do not have this ambiguity and accept ordinary arrays/objects.

Collection payloads count toward static rule expansion, per-boundary notifications, VM rule-write and headless trace allowances. The 16 KiB per-value bound keeps collection writes within the existing string-sized dispatch envelope. Accounting bounds retained payload, not allocator overhead. Scalars retain their existing limits and JSON encoding. State notifications retain existing value equality; checkpoint comparisons additionally check float bits, including signed zero within collections.

## Saves and compatibility

Save format 1 remains unchanged: arrays/objects are values and a root `null` still means a removed state declaration. Map order, scalar types and finite float bits survive save round trips. No migrations are introduced. Authors must bump Story `saveSchema` when changing value kinds or replay mechanics; [ADR 0007](./0007-safe-checkpoint-saves.md) continues to define eligibility and candidate isolation.

The existing 4096-entry/8 MiB save limits still apply. Checkpoint capture and display arming check Persistent payload limits before cloning frozen/expected snapshots; an oversized capture yields a save-data refusal without failing ordinary dialogue. The bounded save writer remains the final check on actual encoded size, including JSON escaping and envelope overhead. Failed candidates preserve live state and existing slot/backup files.

## Verification

[The collection demo](../../examples/collections-demo/README.md) exercises whole-value command writes, scalar views, reset and checkpoint replay. API/Core/save/Story/CLI/checkpoint tests cover canonical ordering, scalar compatibility, flat/size/type rejection, source diagnostics, expansion accounting and candidate isolation. Native player behavior remains queued in issue #5; it does not block implementation or establish a Phase 4 gate.
