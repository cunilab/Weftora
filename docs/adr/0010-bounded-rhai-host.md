---
id: WFT-ADR-010
title: ADR 0010 — Bounded Rhai Host
status: proposed
---

# ADR 0010 — Bounded Rhai Host

## Decision

Add `weftora-script`, the only adapter owning Rhai types and its pinned `=1.26.1` dependency. Story/API carry plain source and permission metadata; Core, Flow, Story and Save remain transitively free of Rhai. `weftora-yarn::DialogueRunner` composes this adapter so CLI play, graphical play and checkpoint reconstruction use one execution boundary. This is a narrow exception to apps-only adapter composition. Cached ASTs and the host stay thread confined in the existing NonSend runner; shared Rules contain no Rhai engine or AST.

Require `rules.rhai/1` alongside existing command/event capabilities. A command declares exactly one of `actions` or `script`. A script declaration specifies a confined Story-relative `file`, a `function`, and explicit `reads`, `writes` and `signals` allowlists. Its function receives a state handle followed by the command's typed scalar arguments. No top-level statements or anonymous functions are accepted. Check/export compile every script and check the function's arity before activation. Export ships source, with no new compiled artifact or save format.

Sources are limited to 64 KiB each, 64 script commands and 1 MiB total source (counting each command). Permissions are unique canonical identifiers, at most 256 per list and 512 bytes each. State keys must be declared, with bounded defaults; saved Stories can access only Persistent state. Scripts cannot emit `state.changed` or its descendants. Runtime checks enforce these restrictions even if metadata/state is constructed outside the Story loader.

## Host contract

The handle exposes `get`, `set`, `add`, `reset`, `remove` and `emit`. Reads return copies; local list/map edits reach canonical state only through `set`. Arrays provide `len`, `push`, `contains` and iteration; maps provide indexing/property access, `len`, `contains` and `keys`; strings provide byte `len`. Canonical writes require the existing flat scalar, finite numeric, fixed root kind, 256-entry and 16 KiB payload rules. Host handles, characters, unit, functions and nested collections cannot enter state. No default packages are installed.

A raw engine has integer/float/string operators and explicitly registered helpers. Builds disable modules, time, closures, optimization and custom syntax. Compilation rejects dynamic evaluation, function pointers, reflection, print/debug and module operations, including method syntax. No filesystem, network, process, clock or ambient random API is registered. Canonical seeded randomness is an additive extension defined in [ADR 0011](./0011-canonical-seeded-randomness.md). A fresh engine and scope execute each call against a cached function AST; nothing persists outside canonical state.

Each Flow dispatch and its state-change feedback share a 10,000-operation script allowance. This allowance resets at the next independent dispatch, not every script in its feedback chain. The runner separately caps consecutive commands between blocking displays; the interpreter allowance is not a wall-clock deadline per display. Every attempted host mutation/emission also consumes ordinary Flow work; queued emissions reserve and consume queue work. Limits additionally cover call depth 8, expression depth 16, 256 locals, 64 functions, 16 KiB strings and 256 array/map entries. Exceeding them produces a Story diagnostic. Operation counts measure interpreter work, not elapsed CPU time or total allocator overhead; these bounds do not certify hostile mods as safe.

Execution temporarily moves State into a thread-confined host context and restores it through a drop guard. It never reacquires the shared-state write lock. Successful emissions join Flow's existing FIFO queue. On failure, completed writes remain, queued emissions are discarded and source diagnostics retain the innermost script line. Checkpoint reconstruction runs against its isolated candidate; failure never publishes that state or replaces save files. Ordinary UI mutations require another checkpoint before saving, as before.

## Remaining scope

This first slice supports canonical state and signals. Script-internal Engine command calls remain deferred. Canonical seeded randomness is now implemented under the explicit replay contract and tests in ADR 0011. Arbitrary external host-write bridging and native acceptance also remain open. [Scripts demo](../../examples/scripts-demo/README.md) and [Phase 4 evidence](../evidence/phase-4.md) record the implemented checks.

## Dependency exception

Rhai 1.26.1 requires smartstring 1.0.1 with no feature to replace it. RustSec RUSTSEC-2026-0249 marks all smartstring versions unmaintained after its repository was archived; it lists no patched release. `deny.toml` records a specific maintenance-advisory exception, following the existing dependency policy. This is maintenance risk, not evidence of a reported memory-safety vulnerability. Re-evaluate the dependency on Rhai upgrades; runtime bounds do not remove the need for upstream fixes.

## References

- [Rhai raw engine](https://rhai.rs/book/engine/raw.html)
- [Rhai safety limits](https://rhai.rs/book/safety/index.html)
- [Rhai AST API](https://docs.rs/rhai/1.26.1/rhai/struct.AST.html)
