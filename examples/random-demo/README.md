# Seeded Random Demo

One Story-owned Persistent integer (`rng`, default 0) drives a reproducible dice stream. The script declares that key as `random` with read/write permission and calls `state.random(1, 6)`. Rolls accumulate in canonical list state; `last` provides the scalar Yarn/UI view. No new Engine-specific dice command is needed.

```sh
weftora check examples/random-demo
weftora run examples/random-demo --headless
weftora export examples/random-demo --out /tmp/random-preview
/tmp/random-preview/weftora-player /tmp/random-preview/story
```

With matching built CLI/player, fresh dialogue shows `Roll 2`, `Roll 1`, `Roll 2`. Save/F5 at any display, quit/relaunch, then Load/F9: that display and every later roll must match uninterrupted play. Repeated load/resave preserves slot bytes. Saves record the state before the checkpoint prefix; inspect replayed state rather than treating the save as a screenshot of the display.

At the first display, Save then click **Roll again**: the live `last` binding changes to 1 while the original interpolated dialogue stays `Roll 2`. Saving now refuses. Load restores the first roll and stream, then Next produces 1 again. Test mouse and keyboard activation plus resize/DPI layout. These are queued native checks, not observed PASS results.

For a content-only variant, change only manifest `rng.default` to 1, bump `saveSchema` to 2, and re-export with the same player. Fresh dice output changes to `Roll 6`, `Roll 2`, `Roll 1`. Player and compiled dialogue hashes must remain unchanged. Old-schema saves refuse safely. For a deliberately failing replay fixture, use another disposable copy with seed 1 but retain schema 1 to exercise failure rather than migration refusal. Insert `if state.get("rng") == -7046029254386353131 { state.get("denied"); }` immediately after the draw in `roll.rhai`. A fresh launch shows 6; loading the preserved seed-0 first-checkpoint save fails at line 3 and keeps that live display/stream and slot/backup bytes intact. Clicking Roll again afterward yields 2, proving live continuation. This intentionally incompatible schema-1 fixture is only for negative testing; production mechanics changes require the schema bump above. Use the matching CLI to re-export valid edits; avoid overwriting baseline saves.

The algorithm/range mapping, permissions and work accounting are fixed by [ADR 0011](../../docs/adr/0011-canonical-seeded-randomness.md). No automatic seeding, float distributions or Yarn built-in random support is added. Windows testing is tracked in [issue #5](https://github.com/cunilab/Weftora/issues/5).
