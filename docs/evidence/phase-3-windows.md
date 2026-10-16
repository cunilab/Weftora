---
id: WFT-EVID-008
title: Phase 3 Windows Manual Verification
status: proposed
---

# Phase 3 Windows Manual Verification

All native results remain maintainer-owned in [issue #5](https://github.com/cunilab/Weftora/issues/5). This document provides steps and an empty record. Automated CI, headless state tests and package hashes do not prove Windows visuals, hearing, input timing or an independent author's walkthrough. No Phase 3 gate is claimed.

Use the Story A artifact from the exact PR/commit under test. Keep its baseline folder intact, including story and compiled dialogue. Launch baseline/weftora-player.exe; use the CLI beside it for content previews and re-exporting edited Yarn. The package also contains two edited content variants and same-player-proof.json. Record commit/profile/hashes and the actual device only after testing.

## P3-01: Complete Story A

1. Launch the baseline. The title menu offers Begin and About. Visit About and return.
2. Begin; Mara and Ivo appear with looping music/ambience. Continue to the repair/roof decision.
3. Choose repair. Check Ivo's route, the effect tone, dawn CG and repair-specific reply. Route should read repair and sent-signal true by the ending.
4. Relaunch and choose roof. Check Mara's movement, the effect tone, dawn CG and different reply. Route should read roof and sent-signal true.
5. Check Volume controls while music plays; close the overlay. Confirm the quiet tones are audible on the actual device.

## P3-02 and P3-06: Same player, edits and save compatibility

1. Keep a copy of the Story A test save before testing variants; all variants deliberately share its Story ID.
2. Compare the three player hashes in same-player-proof.json. Run baseline's player against edit-dialogue-screen/story, then edit-theme-image/story. Both use that exact executable.
3. Verify the first variant's edited title/Arrival sentence, then the second variant's changed panel and day-image appearance. Capture the before/after window and actual player hash.
4. Save the first Arrival line in baseline. Load it in the text-patched variant; the edited line should appear and the route/state should be retained.
5. On a separate source copy, remove/rename scene.arrival and re-export with the same CLI/player. Loading the original save must refuse without changing the menu/live state or slot bytes.
6. Repeat with saveSchema changed from 1 to 2. There is no migration yet: it must report no migration, preserve live state and leave the original slot unchanged. Do not hand-edit a checksum body to simulate this case.

## P3-03: Input, bindings and resize

1. Complete both routes with mouse input and again with keyboard Tab/Shift+Tab/Enter, Space and digits.
2. Check route and sent-signal bindings after choices. Verify a button activation never also advances the dialogue.
3. Open/close Volume and verify modal blocking/focus restoration.
4. Resize smaller/larger and use the actual Windows DPI setting. Check title, dialogue, choices, buttons and status remain readable and reachable.
5. Press F5/F9 with Space or a simultaneous click. A request may refuse on a progress frame; it must never skip the loaded first display.

## P3-04: Safe-point save, quit, relaunch and load

1. At the first Arrival line, press F5 during its half-second fades. It should refuse until the view/audio transition finishes. Retry after the fades: Saved should appear.
2. Advance to a later line and try F5: it should refuse outside the checkpoint's first display.
3. Quit, relaunch, then F9. The first Arrival display, scene and Persistent state should return; intended music restarts from its beginning.
4. Repeat at the first repair/roof line. The opening increments story.attempts once; use the CLI headless trace or save body to confirm loading/re-saving does not increment it twice.
5. Change a volume Setting before load and confirm it remains. An open Volume overlay should close on F9 and return to the initial screen. F5 while it is open must refuse.
6. Verify the first choice focus and classic/Story text refresh after load, even when loading from the initial display.

## P3-05: User data, corrupt save and preservation

1. Confirm files live in LOCALAPPDATA/Weftora/saves/dev.weftora.story-a, never in the Story folder. Record actual directory and hashes after testing.
2. Save twice and verify save.bak contains the exact previous save.json bytes.
3. Make a separate copy of the test save. While the game is paused, replace save.json with invalid JSON and record its hash.
4. Press F9. A nonfatal corruption message should appear; the current dialogue, route, scene and audio should remain active. The invalid slot's hash must remain unchanged.
5. Restore the copied valid test file and load successfully. A refused save/load must not delete either slot or backup. Also check a missing slot reports clearly without silently starting a new game.

## P3-07: Independent author walkthrough

Someone other than the tutorial author should follow the [author tutorial](../story-a-tutorial.md) using the prebuilt CLI/player and a text/image editor. Record whether dialogue, screen/theme and image edits appeared, the unchanged player hash, unclear steps and any required corrections. Do not claim this from automated or agent-scripted checks.

## Empty result record

Copy one record per check into issue #5 after testing. Every actual/device/result field below is intentionally blank.

    check:
    commit:
    build_profile:
    player_sha256:
    cli_sha256:
    windows_build:
    cpu:
    gpu:
    gpu_driver:
    display_resolution:
    display_scale:
    audio_device:
    audio_driver:
    fixture:
    steps:
    expected:
    actual:
    result:
    save_directory:
    slot_sha256_before:
    slot_sha256_after:
    backup_sha256:
    startup_ms:
    ui_latency_ms:
    idle_memory_mib:
    check_ms:
    logs_or_artifacts:
    tested_by:
    tested_at:

## Agent-run records (delegated)

Machine-checkable Phase 3 checks run by the agent on the maintainer's machine under the 2026-10-16
delegation ("test everything you can do without me"). Full method, frames and hashes:
[phase-3-windows-agent.md](./phase-3-windows-agent.md). The native/human items above stay
maintainer-owned; no Phase 3 gate is claimed here.

```yaml
check: P3-02 same-player edits (delegated)
commit: 48a965b
build_profile: release
player_sha256: 0689bd5829b55c7d28aa53ce8003318c27e471ead53617df9b4b6e1b7521d26f
cli_sha256: 4ee2954eb3e527102b4dbd45306ec7ec03c0e6d8e610737ddbe972e9fc76858d
windows_build: 11 Pro 10.0.26300
fixture: examples/story-a via scripts/verify-story-a-edits.py, baseline + edit-dialogue-screen + edit-theme-image
steps: >-
  run the proof script; hash all three exports; launch baseline player, save at first Arrival;
  run baseline player against both variant story dirs; F9 the arrival save in the text-patched variant
expected: >-
  one player hash across exports; edited banner and sentence render; baseline save loads with edited
  line and intact state; changed panel theme and day image visible; load does not change save bytes
actual: >-
  proof script exit 0: three validated exports, identical player 0689bd58…, dialogue/screen/image
  hashes differ per variant, "baseline Arrival save created", text-patch load OK with save hash
  unchanged. Real windows: banner "LAST SIGNAL / EDIT 1"; F9 -> "Loaded." with
  "The ferry sails at sunrise. If the beacon stays dark, they will pass us." and
  Route: undecided / Signal sent: false retained; theme variant shows recolored panel + replaced day image.
result: PASS
logs_or_artifacts: docs/evidence/artifacts/phase-3-windows-agent/edit1-menu-banner.png, edit1-loaded-sentence.png, edit2-theme-image.png
tested_by: agent on maintainer's machine (delegated by Andres-Holivin)
tested_at: 2026-10-16
```

```yaml
check: P3-04 save/load behavior (delegated)
commit: 48a965b
build_profile: release
windows_build: 11 Pro 10.0.26300
fixture: exported Story A baseline player, real %LOCALAPPDATA% slot
steps: >-
  F5 during a view transition, at first display after fades, at a later line, with an overlay open;
  quit/relaunch/F9 at line, repair and menu checkpoints; F5 at menu then story progress; volume change
  then load; F9 with overlay open; finish after load and inspect save bodies
expected: >-
  refusals with reasons outside the save point; restore returns the exact first display with scene and
  state; attempts increments once; Settings survive load; overlay load returns to base; menu load
  refreshes choice text
actual: >-
  messages observed verbatim: "Save/load: wait for the view/audio transition before saving" (mid 1000ms
  move), "Save/load: save is available only at the first line or choice after a checkpoint",
  "Save/load: close overlays and return to the initial screen before saving", "Saved.", "Loaded.".
  Restore frames show the exact checkpoint first line with Route/Signal state rebuilt; save bodies:
  repair attempts 0.0 (checkpoint-boundary), after load -> finish -> re-save attempts 1.0 (never 2.0);
  master volume set 0 stayed 0 after F9 and reopened; F9 with Volume open returned to initial base with
  focus reset; menu-checkpoint load rebuilt "1. Begin / 2. About".
result: PASS
logs_or_artifacts: docs/evidence/artifacts/phase-3-windows-agent/f9-loaded-repair-restore.png, f5-refuse-mid-transition.png, f5-refuse-later-line.png, f5-refuse-overlay-open.png, master-0-retained-after-load.png, f9-menu-choice-refresh.png
tested_by: agent on maintainer's machine (delegated by Andres-Holivin)
tested_at: 2026-10-16
```

```yaml
check: P3-05 user data and failure behavior (delegated)
commit: 48a965b
build_profile: release
windows_build: 11 Pro 10.0.26300
save_directory: C:\Users\andre\AppData\Local\Weftora\saves\dev.weftora.story-a
fixture: real player saves plus agent-written corrupt bytes and a removed directory
steps: >-
  save twice across runs; hash slot and backup; replace save.json with invalid JSON, F9; restore bytes,
  F9; move the directory away, F9; confirm no silent slot creation
expected: >-
  saves at the documented path; backup equals previous slot bytes; corrupt load is nonfatal and does
  not rewrite the slot; restored file loads; missing slot reports clearly without creating a slot
actual: >-
  save.json/save.bak at the documented LOCALAPPDATA path; save.bak byte-equal to the previous slot
  (verified twice: arrival -> repair, repair -> dawn); corrupt F9 ->
  "Save/load: corrupt save: key must be a string at line 1 column 2", menu alive behind, slot hash
  unchanged (526F50D0… before and after); restored file -> "Loaded." with unchanged bytes; missing
  directory -> "Save/load: no save file exists", no save.json created.
result: PASS
logs_or_artifacts: docs/evidence/artifacts/phase-3-windows-agent/corrupt-save-nonfatal.png, restored-save-loads.png, missing-slot-message.png
tested_by: agent on maintainer's machine (delegated by Andres-Holivin)
tested_at: 2026-10-16
```

```yaml
check: P3-06 compatibility refusals (delegated)
commit: 48a965b
build_profile: release
cli_sha256: 4ee2954eb3e527102b4dbd45306ec7ec03c0e6d8e610737ddbe972e9fc76858d
windows_build: 11 Pro 10.0.26300
fixture: story-a copies with `<<checkpoint scene.arrival>>` removed, and with "saveSchema": 2
steps: >-
  check each fixture; load the baseline arrival save headlessly against each; verify exit codes and
  slot bytes; load the baseline save into the text-patched variant (with P3-02)
expected: valid fixtures check OK; loads refuse with reasons; slot bytes unchanged; patched variant loads
actual: >-
  removed checkpoint: check OK (7 nodes), load exit 1 "save checkpoint 'scene.arrival' is not
  available"; saveSchema 2: check OK, load exit 1 "no save migration from schema 1 to 2"; slot hash
  17379B0C… identical before and after both loads; text-patched variant loads the same save with the
  edited line (P3-02 record); attempts chain stays 1.0 through load/finish/re-save.
result: PASS
logs_or_artifacts: docs/evidence/artifacts/phase-3-windows-agent/compat-refusals.txt
tested_by: agent on maintainer's machine (delegated by Andres-Holivin)
tested_at: 2026-10-16
```

```yaml
check: P3-01 Story A walkthrough, visual/state parts (delegated)
commit: 48a965b
build_profile: release
windows_build: 11 Pro 10.0.26300
fixture: exported Story A baseline, keyboard-only drive with per-step PrintWindow frames
steps: >-
  title menu, About and Return, Begin, both decision branches to dawn and end; headless replay of both
  routes for canonical state
expected: menu offers Begin/About and returns; characters and backgrounds render per node; route and
  signal bindings track choices; each route ends with its own reply
actual: >-
  all observed in frames and traces: About returns to the identical menu; Route flips undecided ->
  repair/roof at the first route line; Signal sent flips true by dawn; distinct replies ("She says she
  heard the whistle…" vs "She saw the rooftop flash…"); end frames show The End with Route/Signal set;
  headless end states attempts=1.0, signal=true on both routes. Audio looping/tones NOT covered here.
result: PARTIAL (audio audition stays with the maintainer)
logs_or_artifacts: docs/evidence/artifacts/phase-3-windows-agent/menu-after-about-return.png, decision-choices.png, dawn-repair-reply.png, dawn-roof-reply.png, headless-route-roof.jsonl
tested_by: agent on maintainer's machine (delegated by Andres-Holivin)
tested_at: 2026-10-16
```

```yaml
check: P3-03 input/focus/resize, keyboard parts (delegated)
commit: 48a965b
build_profile: release
windows_build: 11 Pro 10.0.26300
fixture: exported Story A baseline player
steps: >-
  Tab/Enter through buttons and choices; digit selection; Volume overlay open/block/close; F5/F9 under
  overlay; resize window to 900x600 and back
expected: documented focus order; overlay blocks base input; buttons do not also advance; close restores
  focus; resize reflows readably
actual: >-
  Tab order Continue -> Volume -> Save -> Load; Enter activates without advancing the line; digits
  select choices; overlay focused its first button and blocked a digit probe; Close restored focus to
  Volume (highlighted); load reset focus to Continue; 900x600 reflow kept text and buttons inside the
  panel. Real-mouse completion, DPI scale and same-frame F5+Space/click NOT covered here.
result: PARTIAL (mouse, DPI, combined same-frame input stay with the maintainer)
logs_or_artifacts: docs/evidence/artifacts/phase-3-windows-agent/volume-overlay-open.png, close-focus-restore.png, resize-900x600.png, f9-base-after-overlay.png
tested_by: agent on maintainer's machine (delegated by Andres-Holivin)
tested_at: 2026-10-16
```

## Maintainer records

Filled by or for the maintainer during the paired verification session (2026-10-16), completing the
human-observed halves of the delegated PARTIAL records above.

```yaml
check: P3-01 audio audition and full-route listen (maintainer)
commit: 881b010
build_profile: release
player_sha256: 0689bd5829b55c7d28aa53ce8003318c27e471ead53617df9b4b6e1b7521d26f
windows_build: 11 Pro 10.0.26300
audio_device: Realtek High Definition Audio (default device; no-argument launch of the exported package)
fixture: exported Story A baseline package
steps: >-
  no-argument launch; listen at the title menu; About and Return; Begin to the Decision screen
  (audio intentionally silent there); repair route to The End; Esc; relaunch; roof route to The
  End; Volume overlay: master Mute -> 50% -> 100% and the music row the same way while playing
expected: >-
  music and ambience loop without seams, gaps, stutters, pops or unintended volume jumps; a short
  receiver effect tone in each route at its cue line; every volume change audible immediately
actual: >-
  maintainer walked both routes on the real audio device and reported (verbatim answers) "Q1 good"
  and "Q2 doing good": no unexpected seam/gap/stutter/pop/volume jump beyond the intended fades,
  decision silence and ambience end at dawn; the short tone heard in both routes at their cue
  lines; each master/music Mute/50%/100% change took effect immediately on the playing audio.
result: PASS
logs_or_artifacts: docs/evidence/artifacts/phase-3-windows-agent/ (visual frames from the same routes, agent side)
tested_by: maintainer (Andres-Holivin), agent-recorded
tested_at: 2026-10-16
```

```yaml
check: P3-03 mouse activation, one-activation semantics, decorative clicks (maintainer)
commit: 4982551
build_profile: release
windows_build: 11 Pro 10.0.26300
fixture: exported Story A baseline player, real mouse
steps: >-
  mouse-only session: About, Return, Begin; Continue clicks; clicks on character art and empty
  background; decision choice-row click; Volume overlay music 50% and Close; Save (F5) button at
  the repair first line followed by exactly one Continue click; Esc
expected: >-
  every click activates exactly one thing; decorative media clicks are no-ops; Save shows Saved.
  and does not also advance the line; overlay closes back to dialogue
actual: >-
  maintainer reports (verbatim) "everyting doing great": no misfires in either direction (no
  double advance, no dead clicks); character-art and background clicks were clean no-ops; Save ->
  Saved. then exactly one line advance per Continue click; Volume overlay operated and closed by
  mouse.
result: PASS
tested_by: maintainer (Andres-Holivin), agent-recorded
tested_at: 2026-10-16
```
