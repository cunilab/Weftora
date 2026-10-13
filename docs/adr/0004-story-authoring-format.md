---
id: WFT-ADR-004
title: ADR 0004 — Story Authoring Format
status: accepted
---

# ADR 0004 — Story Authoring Format

## Context

Authors write a Story's manifest, data, catalogs, events and screens by hand. Yarn stays the dialogue language; this ADR covers everything else. Phase 0 started with plain JSON, which has no comments and rejects trailing commas — both constant friction for people editing files by hand. Screens are deeply nested trees, so the format must handle nesting well.

## Options

| Option | For | Against |
| --- | --- | --- |
| Plain JSON | Universal, no new code | No comments, no trailing commas |
| **JSONC (chosen)** | Every JSON file is valid; comments and trailing commas; VS Code validates and autocompletes it against a JSON Schema natively; no implicit typing | Not a formal standard; needs a small pre-pass |
| YAML | Most readable for nested data | Indentation errors and implicit typing (`no` → `false`) confuse authors; Rust YAML crates are in flux |
| TOML | Familiar, great for flat data | Awkward for nested screen trees |
| RON / KDL | Expressive | Unfamiliar to non-Rust authors; weak editor support |

## Decision

Story files other than Yarn are **JSONC**: JSON plus `//` and `/* */` comments and trailing commas.

- File extension `.jsonc`. The manifest is `manifest.jsonc` at the Story root.
- Nothing else is added: no unquoted keys, single quotes or other JSON5 extensions.
- Parsing blanks comments and trailing commas with spaces, then parses the result as JSON, so error line and column numbers point at the author's text and no new dependency is needed (`weftora-story` `jsonc` module).
- JSON Schemas for each file type are generated from the Rust types later, so editors can validate and autocomplete.

## Consequences

- Phase 0's `manifest.json` became `manifest.jsonc`.
- Plain `.json` files are not accepted, so each Story has one obvious format.
- If authors later need something friendlier for large data tables, a superseding ADR can add an import format; it would not replace JSONC.
