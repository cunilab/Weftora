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
