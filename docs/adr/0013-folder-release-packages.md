---
id: WFT-ADR-013
title: ADR 0013 — Folder release packages
status: proposed
---

# ADR 0013 — Folder release packages

## Context

The foundation release needs a package an author can inspect and a standalone game that runs without Rust or Cargo. The Engine already reads Story directories and compiled dialogue from the exported folder layout. Introducing an Engine archive reader would add another path-resolution and resource-limits boundary. [P6-05](../success-criteria.md#phase-6--windows-foundation-release) requires an explicit container decision.

## Decision

The canonical runtime package is an ordinary folder. The Engine accepts extracted folders; it does not open ZIP files, mount archives or extract content. Windows x64 remains the foundation release target under the [platform policy](../platforms.md).

A standalone game keeps the existing export layout:

```text
game/
  weftora-player.exe
  story/
  compiled/dialogue.json
  weftora-export.json
```

Keep the folder together. The player runs the compiled artifact; the game does not need the CLI or a compiler. Export metadata and compatibility checks remain owned by [ADR 0002](./0002-versioning-compatibility.md).

The separate authoring-tools folder contains matching `weftora.exe` and `weftora-player.exe` beside each other, an author README and the reviewed licence notices. Example game folders are exported with that same frozen player. Each example retains its own player so it can run independently. File layout selection does not establish clean-machine acceptance or complete the licence audit.

## GitHub download transport

GitHub Release attachments are files, so distribution may wrap each validated folder in a ZIP: one authoring-tools asset and separate example-game assets. ZIP is download transport, not a Story or Engine runtime format. Users extract a trusted release into a fresh directory and run the folder. The Engine has no archive-extraction fallback.

Release packaging must use only the approved folder inventory, with relative member names under one package root, and refuse absolute/traversal paths, links, junctions/reparse points and unexpected entries. Record uncompressed file-count/size bounds. Extract the produced ZIP into a fresh verification directory and compare file names and SHA-256 hashes with the approved folder before publishing its checksum. Test unsafe member names, links and exceeded bounds in the transport packager; these checks are pending implementation, not passing evidence from this ADR.

The archive-reader traversal/decompression-bomb branch of P6-05 does not apply to the current folder runtime. Any future Engine archive support needs a new ADR, bounded extraction before allocation or writes, and explicit traversal/decompression-bomb rejection fixtures. ZIP transport validation remains a separate mandatory release-packaging task in the [release checklist](../releasing.md#release-checklist).

## Consequences and validation

Folder exports preserve the existing source validation, overlap checks and copied-player identity/hash checks. They allow direct inspection and content edits followed by re-export. More files must stay together, and independent examples duplicate the player; the foundation release accepts that cost rather than introducing an Engine archive layer.

[P6-01/P6-02 automated evidence](../evidence/phase-6.md) verifies the current folder workflow, not the planned ZIP packager. Clean Windows VM, release-profile and native acceptance remain separate. [Releasing](../releasing.md) owns the trunk-based tag/GitHub Release process and the remaining packaging work.
