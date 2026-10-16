---
id: WFT-ADR-011
title: ADR 0011 — Canonical Seeded Randomness
status: proposed
---

# ADR 0011 — Canonical Seeded Randomness

## Decision

Add `rules.random/1` to the bounded Rhai host. A script declares an optional `random` key, includes it in both its `reads` and `writes` permissions, and requires the new capability alongside `rules.rhai/1`. The key must be a declared Persistent integer, even in Stories without saves. It is an ordinary Story-owned state field; the Engine reserves no key and keeps no hidden generator. Its default/current integer bits are the seed/current generator state. Multiple commands may deliberately share a stream by naming the same key, or use separate keys for separate streams.

`state.random(min, max)` returns an integer in the inclusive signed i64 range. Equal bounds still advance the generator. Reversed bounds, missing configuration/permissions, a removed key or an incompatible scope/type yield script diagnostics without advancing it. There is no implicit wall-clock or OS seed. This generator is for reproducible gameplay, not cryptography.

The fixed algorithm is SplitMix64, with unsigned 64-bit wrapping arithmetic: increment state by `0x9e3779b97f4a7c15`; mix with shifts 30/27/31 and multipliers `0xbf58476d1ce4e5b9` and `0x94d049bb133111eb`. Store the updated state bits as an i64. The [public-domain reference implementation](https://prng.di.unimi.it/splitmix64.c) defines the raw word transformation. Seed zero produces raw words `e220a8397b1dcdaf`, `6e789e6aa1b965f4`, `06c45d188009454f`; inclusive dice draws 1–6 start `2, 1, 2, 5, 2, 1`.

For a nonzero unsigned range width `n`, reject raw words below `(-n wrapping) % n`, then return `min + (word % n)` using wrapping bit arithmetic. Width zero represents the full signed range, so use the entire raw word as the offset. This avoids modulo bias and signed-width overflow. Every raw word, including a rejected sample, consumes one ordinary dispatch work unit and writes the updated canonical state. Each state write also produces an ordinary notification; event fan-out and notification limits include rejected samples. Rejection is therefore bounded by the existing work allowance even while inside a native helper. Successful results remain subject to the interpreter's existing operation allowance.

## Replay and failure semantics

The existing save format and checkpoint model are unchanged. Saves contain node-entry state, and reconstruction reruns the checkpoint prefix against an isolated candidate. RNG state before that prefix reproduces its draws; the resulting state continues the same sequence at later checkpoints. Tests compare uninterrupted play with repeated loads/resaves at all three demo checkpoints. UI draws mutate Persistent state and invalidate saving until the next checkpoint, like other UI commands.

Earlier raw draws remain applied if a later primitive fails or work runs out; queued emissions are discarded, matching other command writes. A failed restore advances only candidate state and cannot publish it, alter the live stream or replace slot/backup files. Re-reading state on every helper call makes normal canonical writes/resets authoritative. Authors may deliberately reseed with `state.set`, but must not overwrite the stream field accidentally. Changing seeds, stream ownership, draw ordering, ranges or replay mechanics requires a Story `saveSchema` bump. The v1 algorithm/constants/range mapping cannot change silently; a future change needs a new capability/contract.

## Scope and validation

The implementation lives in the existing script adapter, with plain optional key metadata in API/Story and no new dependency. No state/save/Rhai artifact format changes are introduced. Yarn's standard random functions remain unchanged and prohibited in checkpoint nodes; use a Story command to expose a scalar result. No floating-point random API, distributions, shuffling or automatic seeding are added.

[Random demo](../../examples/random-demo/README.md) covers headless/player commands and checkpoint continuation. Unit tests pin raw vectors, signed range boundaries, rejection/work accounting, permission/scope/type failures and inner source lines. CLI and player model tests cover continuation, byte-identical resaves and failed-candidate isolation. Windows native steps remain queued in issue #5 and do not block development or establish Phase 4 acceptance.
