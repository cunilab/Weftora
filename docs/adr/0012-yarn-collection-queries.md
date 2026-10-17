---
id: WFT-ADR-012
title: ADR 0012 — Yarn Collection Queries
status: proposed
---

# ADR 0012 — Yarn Collection Queries

## Decision

Add four read-only Yarn functions under the additive `dialogue.collections/1` capability. Collection declarations still require `state.collections/1`. Keys are canonical state paths such as `bag.items`, passed as strings; the `$name`/double-underscore variable mapping does not apply to these strings.

| Function | Result |
| --- | --- |
| `state_list_len("items")` | Number of list entries |
| `state_map_len("schedule")` | Number of map entries |
| `state_list_contains("items", needle)` | Whether a list contains the scalar needle |
| `state_map_contains_key("schedule", "morning")` | Whether a map contains that exact string entry key |

Length is at most 256 and therefore exactly representable in Yarn's f32 number type. No collection, iterator or mutable reference crosses the Yarn boundary, and no cached scalar mirror is maintained. Every call reads the current canonical value. Missing/removed collection targets, wrong root kinds, invalid state paths, wrong argument types/counts and exhausted bounds produce controlled errors; an absent list member or map entry returns false. Empty maps may have an empty-string entry key; entry keys are not canonical state paths.

List membership is type-sensitive for bool/string, with no coercion between those kinds and numbers. For numeric needles, the represented Yarn f32 value is compared exactly. Stored integers are compared as integers after checking that the needle is integral and within `[-2^63, 2^63)`; they are never rounded to f32. Stored f64 values compare against the f32 needle promoted to f64, so canonical `0.1` need not equal Yarn `0.1`, while exactly representable `0.5` does. Large integer false matches are forbidden. Needles themselves follow Yarn literal/expression rounding: a literal `16777217` is already `16777216` before the helper sees it. Use Rhai for exact arbitrary i64/f64 comparisons. This query returns a boolean rather than exposing a large numeric value to Yarn.

## Runtime and compiler boundary

The compiler and artifact validator share signature-only library registrations from `weftora-yarn`. The runtime uses the same signatures for arity/return type metadata, with a fallible host handler intercepting these names before coercion or fallback execution. The vendored Yarn runtime gains `continue_with_function_handler`; returning None delegates to the registered function, while an error immediately stops execution, clears pending events and preserves the executing node. Returned values must match the declared return type (or Any). Existing continue APIs keep their registered implementations. The runtime patch depends on no Story/Core types.

Signature implementations fail closed if invoked without the handler; the runner catches this internal misuse as a terminal error. Supported start, choice and restore paths all use the central handler. This avoids error sentinels: a failed query cannot return a plausible zero/false or allow subsequent VM writes. Earlier completed writes remain, and Weftora marks the runner terminal. Query errors carry the query name, source file and node; precise expression lines are not retained in artifact format 1. Compiler signature errors retain compiler file/line diagnostics. [The vendor patch note](../../vendor/yarnspinner_runtime/WEFTORA-PATCH.md) records the API change.

Reads borrow SharedState only during the VM function. The rule/script boundary takes its exclusive borrow after VM execution, so the query does not reacquire that write lock. Lists/maps retain their existing 256-entry/16 KiB canonical bounds; state paths and entry keys are limited to 512 UTF-8 bytes, and scalar string needles to 16 KiB. Length is constant work, map membership uses sorted-map lookup and list membership scans bounded entries. The existing cumulative VM instruction allowance bounds repeated calls; these are not wall-clock or hostile-mod safety guarantees.

## Validation and saves

`check`/export/player validation require the capability for every use, including unreachable calls. Directly pushed literal state keys are checked against declaration kinds and scopes before activation. Computed arguments (including variable keys) are checked at execution; this is not a general bytecode data-flow validator. Saved Stories may query only Persistent targets, for both literal and computed keys, matching the existing save-safe rule input policy. Existing checkpoint restrictions on Session Yarn variables still apply.

Standalone `DialogueRunner::new` supports typed queries against its supplied state without a Story manifest; `for_story` also checks the Story capability and save scope policy. Artifact format 1 and save format 1 remain unchanged: no new serialized function metadata or variable type is introduced. Older players refuse the new capability before activation. Checkpoint candidates read their own canonical state; a failed query cannot publish that candidate or replace live state/view or good save files. UI command mutations remain absent from replay and require a new checkpoint before saving.

## Evidence and remaining scope

[Queries demo](../../examples/queries-demo/README.md) reads collection lengths/membership directly, with no `item_count` state mirror. Adapter tests cover numeric precision, read-only behavior and invalid arguments; compiler/runtime tests cover signatures, artifact reload, dynamic arguments and stopping before later writes. CLI/player tests cover live UI changes, capability/scope/target diagnostics and byte-identical replay/candidate isolation. Windows checks remain queued in issue #5 and do not block development or establish native acceptance.

No collection mutation, arbitrary element getter, query binding language or Story-defined Yarn function registration is added. Those need separate contracts if an actual Story requires them.
