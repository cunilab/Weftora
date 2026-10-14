---
id: WFT-ADR-006
title: ADR 0006 — Safe Checkpoint Saves
status: accepted
---

# ADR 0006 — Safe Checkpoint Saves

## Decision

Use stable author checkpoint IDs, not instruction offsets or compiled-artifact hashes. A literal Yarn command such as `<<checkpoint scene.intro>>` must be the first instruction of its node, with no substitutions. IDs use canonical state-key grammar, are at most 128 bytes and are unique across the Story. The compiled artifact supplies the ID→node map; no duplicate manifest map is introduced. A save-enabled schema-1 Story declares a positive saveSchema and the save/1 capability. This is additive; existing Stories retain Engine API 0.1.0 and manifest behavior.

The runtime captures Persistent state and positive Yarn visit counters at that command, before executing the node's opening. It arms the point only when the opening reaches its first line or choice in the same node. Progression disarms it, and saving refuses while Persistent state differs from its displayed value; Settings changes do not. Saving also requires the host's commands/transitions/async work to be settled. Random functions are forbidden in checkpoint nodes because the current Yarn RNG is not resumable. Authors can use other nodes for random gameplay. Checkpoint nodes cannot read manifest-declared Session variables: load resets them, so their prior values cannot reproduce the opening. Deterministic Session writes remain allowed. Settings reads use current preferences on load; authors must avoid making checkpoint meaning or Persistent outcomes depend on a Settings preference.

A checkpoint opening rebuilds presentation from a fresh view/audio model on load. It must establish its own scene; it cannot depend on the prior renderer's layers, audio position or open overlays. Checkpoint meaning changes require a save-schema bump. Renaming an untracked node can preserve its checkpoint ID; removed or incompatible saved visit-counter names are refused until a migration exists.

## Format 1

The strict JSON envelope contains saveFormat: 1, bodySha256 and a raw JSON body. The checksum is SHA-256 of weftora-save-format-1, NUL, then the body's exact UTF-8 bytes. It detects accidental corruption, including valid JSON value changes; it is not authentication. Future format numbers are rejected before parsing current fields. Output is deterministic and ends with one newline. Exact finite float round trips are enabled explicitly.

The body contains storyId, storyVersion, saveSchema, writerEngineApi, writerEngineRelease, checkpoint: {id}, state and dialogue: {visits}. State values are canonical scalar values or null. Null records an explicitly removed Persistent declaration; omitted keys are compatible additions supplied by the current Story's defaults. Session and Settings values are excluded. Visit counters are integer node-name→count records, bounded to 0..16777216.

Limits: 8 MiB encoded read/write, 4096 state entries, 512-byte state keys, 16 KiB string values, 1024 visit counters and 256-byte node names. Serialization is bounded while writing, not only afterward. Duplicate keys and unknown fields are errors. These are format/resource bounds, not a process-memory budget.

## Restore

1. Validate format, checksum, Story identity, save schema and known checkpoint.
2. Build a fresh candidate from current Story and Yarn defaults; Session starts fresh.
3. Retain current Settings for still-declared Settings keys; apply Persistent entries to the candidate.
4. Validate saved visiting counters and adapter-specific state types/precision.
5. Start the mapped checkpoint node using fresh presentation models and reach its armed first line/choice.
6. Activate the candidate state, dialogue and view together.

Prefix state changes follow the normal notification boundary. Observers that affect checkpoint outcomes must operate on the candidate during validation; side effects must not escape before activation. Headless built-in presentation commands are validated once during priming, then their buffered trace is published without executing them twice.

Unknown/removed saved keys, wrong scopes/types and incompatible schemas are refused. Story and writer version strings are diagnostics, not save-compatibility gates. New Persistent fields keep current defaults. No migration registry or Rhai host is introduced in this slice; incompatible schemas refuse explicitly. Failure preserves the live game and saved file; loading never silently starts a new game or falls back to a backup.

## Storage

One slot, save.json, and the previous file's exact bytes in save.bak, live outside the Story under the user's data directory. Windows uses LOCALAPPDATA/Weftora/saves/Story-ID. Other platforms are development hosts, not release targets. The app validates the path and Story ID; the storage core receives that trusted directory.

Writers take a nonblocking OS file lock on a permanent save.lock inode and explicitly unlock on guard cleanup, including error paths. See [Rust file-lock semantics](https://doc.rust-lang.org/std/fs/struct.File.html#method.unlock). Serialize/validate first; write a same-directory temporary file and sync its contents; publish the previous save through a separate temporary backup; then atomically replace the main slot using tempfile's platform replacement. Slot/backup/lock links and Windows reparse points are refused. Existing slots are streamed into the backup with a separate 64 MiB limit; an even larger existing slot refuses with an actionable error and remains intact. A failed publication leaves the prior main slot intact. The backup may have refreshed before a later main-replacement failure. Atomic visibility and flushed file contents do not imply a filesystem-wide power-loss guarantee.

## Implementation and evidence

The storage slice implements the container and scoped candidate state. Runtime integration now implements checkpoint recognition, frozen node-entry state and visits, actual dialogue eligibility, user-data path resolution and headless save/load. Player controls and atomic view activation follow as separate integration work. No P3-04 or Phase 3 gate is claimed by the storage core alone. Native Windows tests and results remain maintainer-owned under issue #5, with blank records.
