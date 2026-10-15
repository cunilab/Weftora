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

## Views/audio scripted demo

Use the media implementation's `weftora-windows-media-preview-<commit>` CI artifact, or build the same commit and run/export `examples/presentation-demo`. The prior UI-only preview does not include this Story.

1. Record the commit/profile/player hash, Windows/GPU/audio device/driver fields yourself. Launch the exported presentation demo and retain the presentation-request logs.
2. Verify the day background and two differently colored character rectangles. Wait 500 ms for their fades. Use Next; verify a 1.5-second background crossfade and the left character's move/scale/opacity change.
3. Advance through CG and decorative overlay fade-in, then both fade out. Decorative images must not block background clicks or UI buttons.
4. Listen for looping 220 Hz music and 330 Hz ambience, plus one-second 440 Hz voice and 660 Hz effect cues. Listen across several music/ambience wrap boundaries for clean looping without gaps. Open Volume: mute each channel separately; 50%/100% and master gain must change the currently playing cues without restarting them.
5. Advance to music/ambience fade-out. Advance promptly on the voice/effect step to verify their fade-out too. Re-run the Story if a one-shot ended before testing its control. Record actual observations or audio capture; ECS/CI gain arithmetic is not audible evidence.
6. Verify the final character hides, modal keyboard/mouse handling, resizing and controlled failures for invalid assets/volume. Performance measurements require a release build and the existing blank timing/memory fields.

## Runtime error exit scenario

Use the runtime-fix revision's preview or build that revision. In a copied Story, keep syntax valid and add a declared string initialized to "x"; double it fifteen times before the first line. Static checks can accept the program, but playback must display a controlled string-budget error naming its source file/node. Close the window and record the nonzero process exit status. Also close a valid Story normally and with Escape; both should return zero. Record actual/result/artifact fields below only after testing.

## Remaining native scenarios

Run these only after the applicable implementation and demo are available. No result is supplied here.

| Check | Scenario | Actual | Result | Artifact |
| --- | --- | --- | --- | --- |
| P2-01 | Choice → canonical state → UI/view → later Yarn condition | | | |
| P2-02 | Invalid syntax, missing node, unknown command; file/node diagnostics | | | |
| P2-04 | Background, two characters, CG, overlay; position/scale/opacity/fade/move/crossfade | | | |
| P2-05 | Music/effects/ambience/voice; fades and user volume | All four channels (220 Hz music loop, 330 Hz ambience loop, 440 Hz voice, 660 Hz effect) audible on Realtek HD Audio; loops wrap cleanly, fades smooth; per-channel mute/50%/100% and master change the currently playing cue without restart | PASS | [record](#p2-05--musiceffectsvoice-fades-and-user-volume), [demo frames](./artifacts/phase-2-windows-agent/) |
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

## Completed maintainer records

Only checks the maintainer actually ran appear here, filled from their own observation. Everything else stays blank above.

### P2-05 — musiceffectsvoice fades and user volume

```yaml
check: P2-05
commit: c9f2aa0
build_profile: release
player_sha256: 612f956035bcfe57009f28df36234a3e8563900f0d34b31cda0b9b8903d2bfec
cli_sha256: 912dc804a4de43006ad690a4bc44b3f78f75d06708d5f034c8e015e69c0268c6
windows_build: 11 Pro 10.0.26300
cpu: Intel Core i7-13620H
gpu: NVIDIA GeForce RTX 4050 Laptop GPU (+ Intel UHD)
gpu_driver: 610.62 (Vulkan)
audio_device: Realtek High Definition Audio
audio_driver: Windows stock (WASAPI)
fixture: examples/presentation-demo (release export)
steps: >-
  Launch exported presentation-demo; listen through the story while advancing
  (music + ambience loop from the opening, voice/effect one-shots); observe the
  1500 ms music/ambience fade-out and voice/effect fade-in near the end; open the
  Volume overlay and exercise per-channel Mute/50%/100% and master
  Mute/50%/100% against the already-playing cues.
expected: >-
  All four channels audible; loops wrap without click/gap; fades smooth;
  volume/mute changes apply to the playing cue immediately, no restart.
actual: >-
  Maintainer listening test: all four channels heard; fades and loop boundaries
  clean; volume and mute changes affected currently playing cues immediately
  without restarting them. Verbatim answers: "yes all do great" (audibility,
  fades, loops) and "yes do great" (live volume/mute without restart).
result: PASS
startup_ms:
ui_latency_ms:
idle_memory_mib:
logs_or_artifacts: docs/evidence/artifacts/phase-2-windows-agent/
tested_by: Andres-Holivin
tested_at: 2026-10-15
```

### Recheck 1 — no-argument launch, background, dialogue, both branches, chime

Covers "Runnable catalog/artifact recheck" step 1 and the first Phase 2 bullet in issue #5.

```yaml
check: recheck-1 (no-arg launch, branches, chime)
commit: c9f2aa0
build_profile: release
player_sha256: 612f956035bcfe57009f28df36234a3e8563900f0d34b31cda0b9b8903d2bfec
cli_sha256: 912dc804a4de43006ad690a4bc44b3f78f75d06708d5f034c8e015e69c0268c6
windows_build: 11 Pro 10.0.26300
cpu: Intel Core i7-13620H
gpu: NVIDIA GeForce RTX 4050 Laptop GPU (+ Intel UHD)
gpu_driver: 610.62 (Vulkan)
audio_device: Realtek High Definition Audio
audio_driver: Windows stock (WASAPI)
fixture: examples/hello-story (release export, weftora-player.exe launched with zero arguments)
steps: >-
  Double-click-style launch (no CLI arguments, story/ next to the exe); verify
  room background and Guide dialogue; play branch "Yes, show me." to the end
  message; close; relaunch from a fresh process and take "Not right now." to the
  end; press Escape there.
expected: >-
  Background and dialogue visible at start; chime audible (Story plays
  sfx.chime once at start, intro.yarn line 5); both branches reachable from
  fresh launches; end message shown; Escape closes cleanly.
actual: >-
  Maintainer: launch visuals correct (background + dialogue, choice "A"); chime
  heard once at first open and not again — matches the Story source, which
  triggers it exactly once at Start; run 1 took "Yes, show me." and reached the
  Esc-to-quit end message; run 2 (fresh process) took "Not right now.", reached
  the end, Escape closed with no problem. Agent-side close/Escape exit code 0
  was separately measured in the automated evidence (see phase-2-windows-agent).
result: PASS
logs_or_artifacts: docs/evidence/artifacts/phase-2-windows-agent/ (frames hello-0-opening/hello-1-choices/hello-2-curious-true)
tested_by: Andres-Holivin
tested_at: 2026-10-15
```

### Recheck 2+3 — same-player edit and stale-artifact rejection

Covers "Runnable catalog/artifact recheck" steps 2–3 and the corresponding issue #5 bullet.
Agent-run on the maintainer's machine under explicit maintainer delegation ("i think you can do
this without me"); every claim below is machine-verified from a captured frame, a hash or an exit
code, with artifacts attached. No human perception is involved in this check.

```yaml
check: recheck-2+3 (same-player edit, stale artifact)
commit: c9f2aa0
build_profile: release
player_sha256: 612f956035bcfe57009f28df36234a3e8563900f0d34b31cda0b9b8903d2bfec
cli_sha256: 912dc804a4de43006ad690a4bc44b3f78f75d06708d5f034c8e015e69c0268c6
windows_build: 11 Pro 10.0.26300
cpu: Intel Core i7-13620H
gpu: NVIDIA GeForce RTX 4050 Laptop GPU (+ Intel UHD)
gpu_driver: 610.62 (Vulkan)
fixture: hello-story working copy (edited line) + release export (edited in place for stale test)
steps: >-
  Edit one Guide line in a working copy; record the player SHA-256; run CLI
  `run` (compiles a fresh dialogue preview and launches the player); capture
  the window; re-hash the player. Then edit the exported Story's yarn in place
  WITHOUT re-exporting and launch the export; finally re-export into a fresh
  directory and launch again.
expected: >-
  Edited line visible with unchanged player hash; stale export refused with a
  controlled message and nonzero exit; fresh re-export launches.
actual: >-
  Edited line rendered in the player (frame attached); player SHA-256 identical
  before/after (612f9560…, matches the release evidence hash). Stale export
  refused before any window opened: stderr "stale dialogue artifact: inputHash
  does not match the Story sources; use `weftora run` or export again", exit
  code 1 (verified twice: cmd-wrapper LASTEXITCODE and Start-Process ExitCode).
  Fresh re-export launched and rendered the edited line (frame attached).
result: PASS
logs_or_artifacts: docs/evidence/artifacts/phase-2-windows-agent/same-player-stale.txt, hello-edited-line.png, hello-reexport-ok.png
tested_by: agent on maintainer's machine (delegated by Andres-Holivin)
tested_at: 2026-10-15
```

### UI interaction battery — overlay blocking, focus return, Shift+Tab, resize

Covers the issue #5 bullets "Tab/Shift+Tab, Enter/Space, mouse activation and choice digits;
one activation advances one line" (fully) and the Details part of the overlay-blocking bullet.
Agent-driven keyboard/mouse except real-mouse activation, which the maintainer performed and
answered directly (their Q6/Q7): branch choice and Volume overlay were selected by real mouse
click, and a real click on Details opened the overlay. Scripted *injected* clicks fell through
widgets in this harness (reproduced 3x on Details, 1x on a choice row) — a harness limitation,
not a product defect, since real input activates correctly.

```yaml
check: ui-interaction battery (overlay blocking, focus, shift-tab, resize)
commit: c9f2aa0
build_profile: release
player_sha256: 612f956035bcfe57009f28df36234a3e8563900f0d34b31cda0b9b8903d2bfec
windows_build: 11 Pro 10.0.26300
fixture: examples/hello-story (release export)
steps: >-
  Advance to choices; open the Details overlay by keyboard; press choice digit 1
  and click the base while it is open; close it; press Enter on the returned
  focus. Separately: Tab/Shift+Tab focus cycling at line 1; a real-mouse click
  on Details (maintainer); resize to 684x461 and verify rewrap.
expected: >-
  Overlay blocks base digit/click; base unchanged after close; focus returns to
  Details and Enter reopens it; Shift+Tab walks focus backwards; wrapping stays
  readable after resize; real mouse activates buttons and choices.
actual: >-
  Digit 1 + base click while overlay open: base stayed on choices, overlay
  stayed open (blocked). Close: overlay gone, choices + Curious unchanged,
  focus outline on Details. Enter on that focus reopened the overlay.
  (An earlier accidental run additionally showed overlay Set updating both the
  overlay binding and base `Curious` to true in the same frame.) Shift+Tab from
  Details moved focus back to Next. Resize rewrapped text to 3 lines without
  overflow. Maintainer real mouse: branch choice clicked (Q6-A), Volume opened
  by click (Q6-A), Details opened by click (Q7-A). Injected-click fall-through
  noted above as harness-only.
result: PASS (Volume overlay blocking sub-part pending maintainer test)
logs_or_artifacts: docs/evidence/artifacts/phase-2-windows-agent/hello-overlay-blocks-input.png, hello-overlay-closed-focus-returned.png, hello-overlay-refocus-reopens.png, hello-shift-tab-back.png, hello-resize-advance.png
tested_by: agent + maintainer real-mouse checks (Andres-Holivin)
tested_at: 2026-10-15
```

### Volume overlay blocking + decorative click-through — maintainer real input

Covers the issue #5 bullets "Details/Volume overlays block base actions, clicks and choice digits, and
return focus after closing" (Volume part; Details part recorded above) and "Verify decorative media does
not block background clicks, buttons or choices". Performed by the maintainer with their real mouse and
keyboard on the presentation-demo; the agent's injected clicks were not used for these two bullets after
injected input was shown unreliable.

```yaml
check: volume-blocking + decorative click-through (maintainer real input)
commit: c9f2aa0
build_profile: release
player_sha256: 612f956035bcfe57009f28df36234a3e8563900f0d34b31cda0b9b8903d2bfec
windows_build: 11 Pro 10.0.26300
fixture: examples/presentation-demo (release export)
steps: >-
  Open the Volume overlay with the mouse; press Space and click the background
  while it is open; click Close; press Enter. Then advance three lines to the
  CG/tint step and click the tinted background.
expected: >-
  Story input blocked while Volume is open; focus returns to Volume on close
  (Enter reopens it); decorative tint does not block the background click —
  the dialogue advances.
actual: >-
  Maintainer answers (issue #5): Q8-A — while the panel was open, Space and the
  background click did not advance the story; after Close + Enter, the Volume
  panel reopened (focus returned). Q9-A — at the tint step the real background
  click passed through the decorative overlay and advanced the dialogue. Q6/Q7
  earlier confirmed real-mouse activation of choice rows, the Volume button and
  the Details button. This also resolves the agent's earlier synthetic-click
  "discrepancy" as a harness artifact (see phase-2-windows-agent).
result: PASS
logs_or_artifacts: issue #5 answers Q6/Q7/Q8/Q9; docs/evidence/artifacts/phase-2-windows-agent/demo-click-*.png
tested_by: Andres-Holivin
tested_at: 2026-10-15
```

Blank fields and unchecked issue items are not PASS. Completing a phase still requires its required evidence; only P0-10 has the existing explicit gate deferral. Historical evidence and current automated CI results remain intact.
