---
id: WFT-ADR-008
title: ADR 0008 — Declarative Events
status: proposed
---

# ADR 0008 — Declarative Events

## Decision

Add optional manifest v1 `events`, an ordered array of Story-relative JSONC paths. Nonempty references require `rules.events/1`. Each file declares `rulesSchema: 1` and an ordered `events` array. Schema 0 cannot use event files. Engine API 0.1.0 remains unchanged: the capability makes the additive feature explicit and older engines refuse it. No compiled dialogue or save container format changes.

Event objects declare `id`, `trigger`, optional integer `priority` (default zero), optional `condition`, and `actions`. IDs are unique across all files. File order and array order define declaration order. Conditions and initial actions use the [rules contract](../rules.md); unknown fields, variants and duplicate JSON keys are errors. Syntax errors retain source lines; semantic errors include the rule file and JSON-pointer location. The existing confined loader and bounded JSON parser apply.

The shared Story-aware dialogue runner settles rule effects after each VM/presentation event, before consuming an internal checkpoint command or arming a blocking display. It reads newly appended notifications without draining or reordering the host's queue. Each root change dispatches `state.changed` then `state.changed.<key>` if handlers exist; signal payload is the key. New rule writes are processed to a fixed point in queue order. One 1024-unit budget is shared across the root notifications and their feedback for that VM boundary. A new boundary gets a fresh budget, so pre-checkpoint work does not reduce the replay prefix's rule-feedback budget.

This is a boundary after VM execution: Yarn reads and interpolations already evaluated in that VM call do not see the new rule writes. Subsequent calls do. Initial default construction and save-state preparation do not trigger rules. Existing no-rule Stories keep their previous behavior. The plain `DialogueRunner::new` is Story-less; production hosts use `for_story` for initial play and candidate restore.

The current bridge covers changes produced while dialogue executes. External UI/host writes outside that execution are not automatically dispatched, and generic command actions, async work, Rhai and collections remain separate work. Hosts must serialize dialogue and external state access; they must not drain notifications during a VM boundary. Rule settling itself holds the exclusive State borrow. The candidate restore path applies the same semantics without publishing its state, notifications or trace until validation succeeds.

## Save compatibility

Save-enabled rules can reference only Persistent state in keyed triggers, conditions and state action targets. Broad `state.changed` triggers are rejected in those Stories because Session and Settings notifications may differ on load. Explicit signals emitted by deterministic rule actions remain allowed. Non-save Stories may use broad triggers and other scopes.

This keeps rule mechanics a function of the checkpoint's frozen Persistent snapshot; Session reset and current Settings cannot enter rules through the notification bridge. Existing checkpoint restrictions and author obligations in [ADR 0007](./0007-safe-checkpoint-saves.md) still apply, including avoiding Settings-dependent checkpoint meaning. Authors must bump `saveSchema` when edits change checkpoint meaning, mechanics or defaults used by `state.reset`; a file checksum cannot establish semantic compatibility. Future UI/host signal entry points must preserve this save-scope restriction.

## Limits and failures

Rule definitions retain the Flow event/action/condition limits. Story rule strings and referenced string defaults/initial values are limited to 16 KiB, state keys/event IDs to 512 bytes, and trigger/emitted signal IDs to 1024 bytes. New notifications from the current VM boundary are checked before settling and after each dispatch: at most 4096 changes and 32 MiB of key/old/new string payloads. Prior notifications retained by the host do not consume this boundary's allowance; a stale cursor after queue shrink is an explicit error. Rule writes additionally share the existing VM's cumulative write/string budgets between blocking events. A dispatch may temporarily append its bounded 1024-unit workload of bounded literal/default values before accounting. These are bounded staging limits, not a process-memory guarantee.

Failures stop the runner permanently before returning the affected display. Earlier candidate writes are retained within the candidate; failed restore discards that candidate and does not alter the live game, slot or backup. No automatic rollback or migration is introduced.

## Validation

Story-loader negative tests cover capability/schema, confined sources, duplicates, invalid types/actions, source locations and scope/string limits. Real headless replay tests cover ten identical runs, the interpolation boundary, before-checkpoint writes, save/load/resave identity, cycles and failed candidate output/file preservation. A player model test verifies rule-aware restoration and live-state isolation. Native Windows checks remain queued in issue #5 and do not block implementation.

## Addendum: declarative commands and screen input

The additive `rules.commands/1` capability enables optional typed command definitions in the same rule sources. [The command contract](../rules.md#typed-declarative-commands) specifies scalar arguments, static flattening and resource bounds. Yarn, events and screen buttons share the existing Flow primitives; no async Core registry callbacks or script runtime are introduced. Source pointers follow each flattened primitive. Direct command actions complete before queued signals and notification feedback.

Screen state writes use this bridge as well. A Persistent mutation from a screen invalidates the current checkpoint until the next checkpoint, since save replay does not record UI input. This keeps the existing save format and isolated candidate contract. Save-enabled command targets remain Persistent-only; unsupported Session/Settings mechanics fail before activation. Authors bump `saveSchema` when changing replay mechanics.
