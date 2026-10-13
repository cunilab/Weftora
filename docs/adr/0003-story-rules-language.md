---
id: WFT-ADR-003
title: ADR 0003 — Story Rules Language
status: proposed
---

# ADR 0003 — Story Rules Language

## Context

The earlier plan said Story rules would be declarative data only, with a Rhai script host considered "only if declarative rules prove insufficient" in Phase 4. State was limited to bool, int, float and string, and collections were deferred.

That cannot support Story B. A life-sim needs inventories, schedules, loops over locations or characters, arithmetic formulas and helper functions. Expressing these as JSON actions turns the action format into an awkward programming language, and the Engine ends up growing genre features to fill gaps — the exact thing the architecture forbids.

## Decision (proposed)

1. **Declarative actions stay** for simple cases: event triggers, conditions, ordered lists of Engine primitives. They are easy to validate statically and to read.
2. **Rhai is committed as the Tier 2 rules language** from Phase 4. A Story command or event action may call a Rhai function when declarative actions are not enough.
3. **State gains `list` and `map` values** in Phase 4 (before Story B), with deterministic serialisation and Yarn mapping rules.

## Why Rhai

- Pure Rust, embeds without a C toolchain, so the Windows build and future ports stay simple.
- Built-in limits on operations, call depth, string, array and map sizes, and module loading; engines can be built with no packages and no module resolver, so nothing is reachable unless the host registers it.
- Syntax familiar to anyone who knows JavaScript or Rust.
- Alternatives considered: Lua via `mlua` (mature and fast, but adds a C dependency and a larger sandbox surface); a custom expression language (high maintenance, no ecosystem); Yarn's expression language (good for dialogue conditions, too limited for game logic).

## Host contract

The host contract (allowlisted API, what is unreachable, limits, no hidden script state, diagnostics) lives in [architecture §7](../architecture.md#7-rules-scripting-rhai). In short: scripts can only touch state, commands, signals and a seeded random source, and every limit violation is a Story error.

## Consequences

- Rhai is a dependency of `weftora-script` only; headless crates stay free of it.
- The phrase "sandbox" is not used for guarantees: the limits make accidents safe, not hostile mods.
- If Rhai's performance or ergonomics fail Story B's needs, a superseding ADR re-evaluates Lua.

## Validation

Phase 4 and Phase 5 checks in [success criteria](../success-criteria.md) cover limits, error reporting, hostile scripts and Story B's use of lists and maps.
