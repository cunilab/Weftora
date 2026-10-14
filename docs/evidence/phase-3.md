---
id: WFT-EVID-007
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

Windows x64 CI [run 38610052974](https://github.com/cunilab/Weftora/actions/runs/38610052974) passed all three jobs at source revision 8895fd0cd3a85965ade369b6a65afd8559cd5952. The workspace test job passed 216 tests across 25 suites with zero failures; [raw output](phase-3-save-core-windows.txt) preserves the test step. These automated file/codec tests do not prove game save eligibility, player view restoration, native user-data paths or Windows UI. Runtime checkpoints, CLI/player integration, Story A and tutorial remain to implement. All actual Windows device/result/performance fields remain blank and maintainer-owned in issue #5.


## Runtime checkpoint integration

The second slice declares optional positive saveSchema and save/1 without changing Engine API 0.1.0 or existing artifact format. The validated artifact derives stable ID-to-node mappings and rejects nonliteral/nonentry/duplicate checkpoints and random functions in checkpoint nodes. Shared Story validation rejects Session reads in checkpoint nodes. DialogueRunner consumes checkpoints internally, captures pre-prefix Persistent entries and positive visits, and arms only the first same-node blocking display. Progress, valid selection or restart disarms; saving refuses while Persistent differs from the display snapshot. Settings edits and rejected choices/continue calls preserve eligibility.

Headless --save-at stops and writes at that display. --load builds a fresh candidate with current defaults, resets Session, validates saved visits/types/precision, replays the opening once and verifies its exact checkpoint before publishing buffered gameplay events. Normal headless traces remain unchanged. Save directories are resolved outside the package through existing ancestors; Windows uses LOCALAPPDATA, macOS Application Support and Linux XDG data/HOME fallback. Optional --save-dir selects an exact external test directory.

On MacBookAir, full-workspace tests passed 242 tests across 26 suites, with zero failures. Clippy with warnings denied, formatting, dependency and documentation checks passed. Focused regressions cover frozen state, first line/choices, progression/refusals, visit bookkeeping, deterministic prefix replay, byte-identical resaves/backup, both branch traces, text patch compatibility, missing files without creating directories, corruption/schema/checkpoint/prefix failure with unchanged slot and no published trace, and Session read rejection. Claude Opus 5.5 medium Read/Grep review returned SHIP after the Session and rejected-continue fixes; Codex performed all edits/tests.

Player save/load controls, transactional view/audio activation, Story A and the tutorial still follow. Native Windows save paths, player interaction/relaunch/view restoration and author walkthrough remain blank in issue #5. No Phase 3 gate is claimed.

Windows x64 checkpoint-integration CI [run 38614730704](https://github.com/cunilab/Weftora/actions/runs/38614730704) passed all three jobs at source revision ed8d14a8803ac2a87b251ea375dce0bdd94c18bd: 230 tests across 26 suites, zero failures. [Raw test output](phase-3-checkpoints-windows.txt) records the automated run; native player/relaunch tests remain pending.
