---
id: WFT-EVID-006
title: Phase 3 Implementation Evidence
status: proposed
---

# Phase 3 Implementation Evidence

Phase 3 development has started. Phase 2 code is integrated and its main CI passed; native Windows results remain pending in issue #5. No Phase 3 gate is claimed.

## Save storage core

[ADR 0006](../adr/0006-safe-checkpoint-saves.md) defines safe checkpoint replay, format 1 and storage policy. The first implementation slice adds a headless weftora-save crate: deterministic checksummed JSON, format-first rejection, bounded serialization/read, exact finite floats, scoped candidate state, typed errors, an OS writer lock, atomic same-directory replacement and one previous-file backup. Earlier files up to 64 MiB are streamed into the backup; bigger files refuse with recovery information. Slot/backup/lock links and non-regular files are refused. Load never automatically falls back to a backup or overwrites the slot.

Persistent removals remain explicit null entries. Candidate preparation resets Session through the current template, retains still-declared current Settings, and applies known Persistent entries. Unknown keys, wrong scopes/types, missing checkpoints and unsupported schema migrations refuse without activating a candidate. Runtime state/default initialization and dialogue/view validation remain adapter responsibilities in the next slice.

The existing duplicate-key JSON validation pass moved from Story JSONC into Core and is shared by Story and save parsing. The dependency gate now includes the save crate and has a negative Yarn-dependency fixture. No new third-party package/version was introduced; previously locked tempfile, sha2, semver and serde packages are reused. Save explicitly enables serde_json float_roundtrip rather than relying on unrelated feature unification.

On MacBookAir, the full workspace passed 227 tests across 25 suites with zero failures. Workspace Clippy with warnings denied, formatting, dependency direction and documentation checks passed. Regressions cover deterministic round trips, six float edges and 500 generated bit patterns (non-finite patterns skipped), additions/removals/scopes, format/schema/identity/checkpoint refusal, checksum/duplicate/unknown-field corruption, byte/string caps, exact backup bytes, failed backup preservation, missing files, competing writers and explicit unlock with an open duplicated handle. The Unix FIFO check runs in a bounded child process. A parallel lock test exposed an inherited-handle lifetime; explicit guard cleanup and the duplicate-handle regression cover that path.

Claude Opus 5.5 medium was restricted to Read/Grep for advice and independent review. Its storage-core verdict was SHIP after float-roundtrip and typed-error fixes; the focused lock/file-kind follow-up also returned SHIP. Codex performed all edits and tests.

Windows CI evidence will be recorded against an immutable source revision. These automated file/codec tests do not prove game save eligibility, player view restoration, native user-data paths or Windows UI. Runtime checkpoints, CLI/player integration, Story A and tutorial remain to implement. All actual Windows device/result/performance fields remain blank and maintainer-owned in issue #5.
