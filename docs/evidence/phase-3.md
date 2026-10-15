---
id: WFT-EVID-007
title: Phase 3 Implementation Evidence
status: proposed
---

# Phase 3 Implementation Evidence

Phase 3 development has started. Phase 2 code is integrated and its main CI passed; native Windows results remain pending in issue #5. No Phase 3 gate is claimed.

## Save storage core

[ADR 0007](../adr/0007-safe-checkpoint-saves.md) defines safe checkpoint replay, format 1 and storage policy. The first implementation slice adds a headless weftora-save crate: deterministic checksummed JSON, format-first rejection, bounded serialization/read, exact finite floats, scoped candidate state, typed errors, an OS writer lock, atomic same-directory replacement and one previous-file backup. Earlier files up to 64 MiB are streamed into the backup; bigger files refuse with recovery information. Slot/backup/lock links and non-regular files are refused. Load never automatically falls back to a backup or overwrites the slot.

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

## Player save/load integration

The player now exposes save/load buttons through validated Story actions and F5/F9 shortcuts. Requests run after normal frame inputs/actions. Save refuses progress/commands in the same frame, active view/pose/audio fades, open overlays/non-initial base screens and invalid checkpoint data. Music and one-shots may continue playing. Errors use a separate nonfatal status line; load is unavailable after a terminal Story error.

Restore builds an isolated default-initialized candidate with current Settings and reset Session, restores visits, primes its checkpoint presenter, and validates all screen/state/text/choice/volume constraints. Media stages a fresh scene while sharing immutable caches and retaining the audio-token sequence. Live state, dialogue and media replace together only after decoding succeeds. UI layers, choice focus and classic-panel dirtiness reset even when presenter revision numbers match. Existing synchronization stops old sinks; playback cursors are never serialized.

The full workspace passed 248 tests across 26 suites before the final save-time binding hardening. After that addition, all 41 renderer tests passed, including the new regression, and workspace Clippy with warnings denied passed again. The existing dependency/docs/format checks passed. Regressions include settings retention/session reset, first-display replay, byte-identical resave, audio token/entity handoff, nonfatal corrupt-file/bound-text/decode failure with old state/view/audio and slot preserved, in-flight/overlay/changed-Persistent refusal, F5/F9 with Space and same-frame progress, UI capability validation and refusing a hidden invalid binding before overwriting a good slot.

Claude Opus 5.5 medium Read/Grep advice and review returned SHIP with a recommendation to validate hidden bindings before saving. Codex added that check and its preservation test. Native Windows graphical/relaunch checks remain pending in issue #5. Story A, same-player edit proof and tutorial are the next slice; no Phase 3 gate is claimed.

Windows x64 player-integration CI [run 38674201038](https://github.com/cunilab/Weftora/actions/runs/38674201038) passed all three jobs at source revision f372373: 237 tests across 26 suites, zero failures. [Raw output](phase-3-player-windows.txt) includes the player round-trip/negative/sink handoff tests. The focused post-hardening Claude review also returned SHIP. Native graphical/relaunch results remain pending.

## Story A, authoring and content-only edits

Last Signal is a complete small Story A with an immediate Begin/About menu, repair and rooftop routes, distinct dawn replies, character and CG slots, music/ambience/effects, transitions, themed choices, state bindings and six checkpoint IDs. It lives separately in examples/story-a so the earlier Hello Story golden fixtures remain unchanged. CC0 rectangles and quiet tones are prototype assets.

Each checkpoint opening explicitly clears unwanted slots/audio and defines the required characters/poses. A renderer regression drives live play to every ID (including both dawn routes), settles the models, saves, progresses, loads and compares Views.samples, audio channel/asset/envelope, Persistent state and the displayed dialogue. This caught and fixed actors inherited from prior scenes.

On MacBookAir, the final full workspace passed 252 tests across 27 suites, zero failures. Clippy with warnings denied, formatting, dependency and docs checks passed. Real Story branch saves re-save byte-identically and execute the opening's attempt increment once.

The [committed content-edit proof](phase-3-story-a-edit-proof-macos.json) records three exports made with existing binaries, without any Rust build between edits: baseline, dialogue/screen edit, then theme/image edit. All exported players validate and share SHA-256 c4dfc981ce908d4ad7a7e21fc174ba5878f0db5601f781925848ce3138b43208. A baseline Arrival save loads into both edited variants with the new line, and its on-disk hash stays unchanged. This Mac binary is separate from Windows CI binaries. The [proof script](../../scripts/verify-story-a-edits.py) keeps one baseline executable and matching CLI, two edited content packages, a compatibility test save, report and support docs.

Windows CI now pins Python 3.13 and packages that proof as a Story A preview artifact. [The author tutorial](../story-a-tutorial.md) uses prebuilt tools for dialogue, screen/theme and image edits and export. [Windows steps and empty records](phase-3-windows.md) cover P3-01 through P3-07; issue #5 remains the single native reminder. An independent author still needs to perform the tutorial.

A Mac development launch initialized the Metal renderer, window and initial scene commands. Computer Use could not inspect it because the server/client versions differ; no actual visual/input/audio check is claimed from that launch. All native Windows and independent-author results remain blank. Phase 3 implementation is in review; its human acceptance gate remains open.

Claude Opus 5.5 medium Read/Grep acceptance review returned SHIP after the explicit scene-opening and all-checkpoint model-comparison fixes. Codex performed the implementation and verification. The final Clippy run completed successfully.

Windows x64 Story A CI [run 38676278587](https://github.com/cunilab/Weftora/actions/runs/38676278587) passed all three jobs: 240 tests across 27 suites, zero failures, plus formatting, Clippy, dependency, docs, export and all three content-proof validations. Source revision ca888c7dcfd97579b877ad219ebc33448198ea8c and CI merge revision 773f23b1fceeb3a38e98b29af76ffeb2f261c7d9 have the identical file tree c35ab9f3675def1e0bd9eb1223756b423f110563. [Raw tests](phase-3-story-a-windows.txt) include all-checkpoint view/audio restoration.

The [Windows proof JSON](phase-3-story-a-edit-proof-windows.json) records player SHA-256 45be01826fee816249551d764a1a278a70c4396137c73a5535c591aeb24a8afd and CLI SHA-256 af05c00d183848afc07306ad9d7ba02979f1b96f21148b0b2f7a0b59605940b1. Downloaded [Story A artifact 11946542809](https://github.com/cunilab/Weftora/actions/runs/38676278587/artifacts/11946542809) (38,132,050 bytes) was independently checked: retained player/CLI bytes, every dialogue/screen/image hash, baseline compatibility save, both patched first-line results and support docs match the report. The baseline save bytes also match the Mac proof. Duplicate variant executables are absent as intended; both variants use the retained baseline player.

These are automated Windows results. The native Windows UI/audio/relaunch/performance and independent-author records remain blank in issue #5, whose previous content was preserved when adding the Phase 3 handoff.
