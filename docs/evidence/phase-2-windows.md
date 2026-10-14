---
id: WFT-EVID-005
title: Phase 2 Windows Manual Verification
status: proposed
---

# Phase 2 Windows Manual Verification

The maintainer will run Windows manual checks and update this record personally. [Issue #5](https://github.com/cunilab/Weftora/issues/5) tracks all Windows manual work and the original clean-machine export requirement. Device and actual-result fields are intentionally blank. Automated Windows CI results are recorded separately in [Phase 2 implementation evidence](./phase-2.md); they do not fill in this record.

## Build or download a preview

Use the `weftora-windows-preview-<commit>` artifact from the applicable CI run, or build the same commit on Windows:

```powershell
cargo build -p weftora-cli -p weftora-player --locked
.\target\debug\weftora.exe check examples\hello-story
.\target\debug\weftora.exe run examples\hello-story
.\target\debug\weftora.exe export examples\hello-story --out <empty-export-folder>
Get-FileHash <empty-export-folder>\weftora-player.exe -Algorithm SHA256
```

The CI preview is a debug build. Use a release build for performance measurements; record the profile and exact commit/hash. Rust may be installed for Phase 2 testing. The clean-machine test in issue #5 is a separate export/release requirement.

## Runnable catalog/artifact recheck

1. Launch the exported player without arguments. Check the original room background, chime, Guide lines and both choice branches.
2. Record the player hash, edit a Yarn line, then use CLI `run` to compile a preview. Verify the new text and unchanged player hash.
3. Edit exported Yarn without re-exporting. Verify the player reports a stale artifact; re-export into a fresh directory and launch again.
4. Confirm declared image/audio catalog IDs resolve to the original assets. Invalid/missing IDs and category mismatches should fail through `check` before a player starts.
5. Record logs and an actual rendered frame, along with the device fields below.

## Declarative UI scenario

Use the UI implementation's exact CI commit/preview, not the earlier asset-only preview.

1. Launch Hello Story. Verify the room image, speaker/text, choice list, Next/Details buttons and `curious` label are readable; resize narrow/wide and verify wrapping and clickable bounds.
2. Use Tab/Shift+Tab and Enter/Space. One Next activation must advance one line. Details must open the overlay without advancing dialogue.
3. While Details is open, click outside its panel and press choice digits. The base dialogue must stay unchanged. Set curious to true; both state labels should update. Close and verify keyboard focus returns to Details. After closing with the mouse, Space activates that focused Details button again; click the background or focus Next to continue dialogue.
4. Advance to choices. Verify first-choice focus, keyboard/mouse selection, state update and later Yarn condition. Repeat both branches from a fresh launch.
5. Add a licensed valid font to a `fonts` catalog and set a widget's `font` ID; verify actual glyph rendering. Repeat with corrupt font bytes and verify the controlled Story error panel.
6. Try an invalid widget, missing asset ID, unknown screen and wrong `state.set` type. `check` must reject each with its screen filename/pointer before launching the player. Remove screens/initialScreen and their capability from a copied package to recheck the legacy panel.
7. Record actual/result/artifact fields yourself below; keep blank until tested.

## Remaining native scenarios

Run these only after the applicable implementation and demo are available. No result is supplied here.

| Check | Scenario | Actual | Result | Artifact |
| --- | --- | --- | --- | --- |
| P2-01 | Choice → canonical state → UI/view → later Yarn condition | | | |
| P2-02 | Invalid syntax, missing node, unknown command; file/node diagnostics | | | |
| P2-04 | Background, two characters, CG, overlay; position/scale/opacity/fade/move/crossfade | | | |
| P2-05 | Music/effects/ambience/voice; fades and user volume | | | |
| P2-06 | All widgets, live bindings, keyboard/mouse focus, resize and invalid-screen rejection | | | |
| P2-09 | Actual image/audio/font catalog use, including font decoding/rendering | | | |
| P2-08 | Release startup, UI latency and idle memory; compare PRD R15 | | | |

## Maintainer record — intentionally blank

```yaml
check:
commit:
build_profile:
player_sha256:
cli_sha256:
windows_build:
cpu:
gpu:
gpu_driver:
audio_device:
audio_driver:
fixture:
steps:
expected:
actual:
result:
startup_ms:
ui_latency_ms:
idle_memory_mib:
logs_or_artifacts:
tested_by:
tested_at:
```

Blank fields and unchecked issue items are not PASS. Completing a phase still requires its required evidence; only P0-10 has the existing explicit gate deferral. Historical evidence and current automated CI results remain intact.
