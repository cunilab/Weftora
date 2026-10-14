---
id: WFT-MEDIA-001
title: Presentation primitives v1
status: proposed
---

# Presentation primitives v1

The `presentation.media/1` capability enables built-in literal Yarn commands for views and audio. The legacy `background <image>` and `sound <audio>` commands still work without it. Static validation, headless execution and the player use the same command parser. Asset IDs resolve through typed catalogs (or compatible legacy untyped assets), with bounded confined reads and decoder checks in the adapter.

## Commands

| Command | Arguments | Behavior |
| --- | --- | --- |
| `background` | image | Cut to a full-viewport background |
| `show` | slot image ms | Fade in an empty slot; crossfade an occupied slot; 0 cuts |
| `hide` | slot ms | Fade out and clear; empty slots are harmless |
| `move` | slot x y scale opacity ms | Linear transform tween; an empty slot sets its pose immediately |
| `sound` | audio | Effects one-shot with no fade |
| `music` | audio-or-none ms | Loop; replace by crossfade; same currently playing ID does not restart |
| `ambience` | audio-or-none ms | Same looping ownership as music |
| `voice` | audio-or-none ms | One-shot; replace or fade out the previous voice |
| `effect` | audio-or-none ms | One-shot fade-in; `none` fades out active effects |

Slots are `background`, `left`, `right`, `cg`, `overlay`, drawn in that order below declarative UI. The overlay slot is decorative and ignores picking; a declarative overlay is a separate modal UI layer. `none` is a stop sentinel for the new audio commands; use legacy `sound` to play an asset whose ID is literally `none`.

All arguments are literal. Durations are integer milliseconds 0–10000. Positions are normalized viewport center coordinates in -1–2 (0.5/0.5 is the center); scale is greater than 0 and at most 4; opacity is 0–1. Every number must be finite. Background and CG fill the viewport, then apply scale. Character and decorative overlay sizes use native image pixels at a 1280×720 reference, scaled to fit the current viewport and the command's scale. The anchor is always the image center.

Transitions use linear interpolation on the player's clock. Commands in a single dialogue batch share one timestamp and apply in order. Retargeting starts from the currently sampled pose/gain. Each view slot retains at most one outgoing image; a third image replacement drops the oldest outgoing image. Crossfades overlap outgoing and incoming alpha ramps. Hide clears images at its deadline. Transitions do not block dialogue or skip when the user advances. Shake, rotation, easing and wait-for-transition are not part of this contract.

## Audio and canonical user volume

Music/ambience stream through Rodio's built-in looping decoder, which rewinds the immutable encoded cursor instead of buffering a whole decoded track. Bevy plays that source once; the source itself loops. Voice/effects stream once. Loop boundary audio behavior remains a native Windows check. Music, ambience and voice retain at most two cues during replacement. Effects retain at most eight; a ninth stops the oldest. Replacing a channel during a crossfade drops the previous outgoing cue. Natural completion removes a one-shot's entity and source reference. No audio device or a sink appearing after spawn is tolerated; no audible output is claimed in that case.

Optional canonical keys `settings.audio.master`, `settings.audio.music`, `settings.audio.effects`, `settings.audio.ambience`, `settings.audio.voice` must be finite floats in 0–1 with Settings scope. An absent key has neutral gain 1. Declare them in manifest state when exposing volume controls:

```json
"state": {
  "settings.audio.master": {"default": 0.5, "scope": "settings"},
  "settings.audio.music": {"default": 1.0, "scope": "settings"}
}
```

The adapter interprets these known keys only when the capability is enabled. It does not reserve an entire namespace or create another settings store. CLI checks manifest and Yarn-initialized values and literal UI actions; headless also checks each live event; runtime checks live values. Existing UI buttons can set the keys, as the [demo volume overlay](../examples/presentation-demo/ui/volume.jsonc) does. The adapter copies values under one short state read lock, releases it, then updates playback settings and sinks. It leaves state notifications intact.

Effective gain is `master × channel × fade envelope`. The first playback settings already include that gain, so a later-created sink does not pop at full volume. Mute means gain zero; the cue continues running and can be unmuted. Persistence of Settings between sessions belongs to Phase 3.

## Bounds and failures

The existing image header limit is 4096 pixels per side and a conservative 64 MiB decode estimate. Active view assets share a 128 MiB estimate; total new image decodes in one frame use that same budget. Active audio bytes and new audio reads per frame each use 64 MiB. Limits count distinct asset IDs, and caches release unused handles. These limits bound asset data, not the engine's entire CPU/GPU memory. A 1080p image counts as about 32 MiB: simultaneous background and CG crossfades can exhaust the view budget before characters are included. The 720p demo stays below it.

Static command errors name the source file/line and Yarn node. Runtime decode/budget/volume failures use the adapter's error panel. The rest of the command batch is skipped, earlier successful view actions remain, and owned audio stops on an error. Synchronous decoding may cause frame hitches; the maintainer's Windows performance baseline must measure it.

## Scripted demo and evidence

[Presentation demo](../examples/presentation-demo/README.md) exercises all slots, position/scale/opacity/fade/move/crossfade, every audio channel, fade-out and canonical user volume. Its shapes and sine tones are generated CC0 fixtures, not final art/music. Headless mode validates and traces commands without simulating animation time, playing audio or running UI actions.

Explicit-time model tests and ECS fixtures verify arithmetic, ownership, draw values, initial gain, canonical mute and allocation caps. Actual rendering/audio/device/input/performance results remain blank for the maintainer under [issue #5](https://github.com/cunilab/Weftora/issues/5) and the [Windows record](./evidence/phase-2-windows.md). No Phase 2 native gate is passed or waived.
