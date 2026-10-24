---
id: WFT-UX-003
title: Player preferences
status: proposed
---

# Player preferences

Preferences opens from the engine title or game menu. Back/Escape returns to that menu and restores its Preferences-button focus. Story input stays suspended; existing audio and fades continue. Controls expose only Settings declared by the current Story: master/music/ambience/effects/voice volume and supported languages. Volume changes use ten-percentage-point steps, clamped to 0–100, with disabled bounds and distinct accessibility labels. Language changes translate the current offer without replaying Story commands. Engine menu chrome remains English.

Normal native launch restores preferences before displaying the title. New Game and loading a checkpoint retain Settings. Story A now starts directly at Arrival; its old Start/About nodes remain for legacy `scene.menu` saves. Its dialogue toolbar contains Continue; preferences and checkpoint operations belong in the engine menu.

## Storage and recovery

Preferences use the existing per-Story user-data directory, alongside but independent of `save.json`: `preferences.json`, `preferences.bak` and `preferences.lock`. See [save storage](./adr/0007-safe-checkpoint-saves.md) for directory resolution. No preferences are written into Story packages. Explicit `--start-node` previews and test preparations use temporary session-only settings and do not read the normal profile.

The versioned JSON profile contains `preferencesFormat: 1`, the exact `storyId`, and a `values` map. Only changed overrides are persisted, including authored changes to declared Settings; untouched defaults can evolve with the Story. Removing an authored Setting override restores its declared default on the next launch. Unknown or incompatible entries remain in the file for other Story revisions but never enter live progress/session state. Explicitly choosing a supported preference replaces that key, even when selecting the currently active default.

Writes are bounded to 64 KiB, use the existing nonblocking writer lock and same-directory atomic publication, and rotate the previous file to `preferences.bak`. A 250 ms throttle limits repeated writes; Back/Escape and confirmed Quit flush pending changes. A write failure leaves changes active in the session, exposes Retry, and stops automatic retries. Forced flushes retry once. If quitting still leaves pending preferences, a second confirmation explicitly offers **Quit without saving preferences**. Cancel returns to the live session.

Malformed, newer-format, foreign-Story, oversized or unsafe files are left untouched. Controls still work temporarily. Reset requires confirmation, publishes defaults before changing live state, and retains an existing original as a unique `preferences.replaced-*.json` that later writes never rotate away. Cancel and failed Reset preserve state. If unsafe file types or very large files prevent safe retention, move the offending preferences file aside manually before retrying; leave `save.json` alone.

Two simultaneous instances use last-writer-wins preferences; the lock prevents overlapping publication, not merging. Atomic publication does not promise survival of every filesystem rename after power loss. Preferences do not introduce slots, rollback, text-speed simulation, skip/auto, or persistent history.

## Verification

Storage tests cover bounded/schema validation, locks, links, backups and Reset cleanup. Renderer tests cover restart restoration, default drift, scope filtering, retained incompatible entries, preview isolation, exact stepping, live localisation, menu focus and failure/quit recovery. Native rendering and Windows acceptance limits are recorded in [Phase 6 evidence](./evidence/phase-6.md#persistent-preferences-and-story-a-start-flow).
