# Collection Queries Demo

Yarn reads list/map length and membership from canonical state, without an `item_count` mirror or collection-valued Yarn variables. Declarative commands still pack/reset whole values; UI bindings use the scalar status.

```sh
weftora check examples/queries-demo
weftora run examples/queries-demo --headless
weftora export examples/queries-demo --out /tmp/queries-preview
/tmp/queries-preview/weftora-player /tmp/queries-preview/story
```

With matching built CLI/player, fresh output is `Packed 2 items`, `Now 2, key true, schedule 3, morning true`, then `Items after reset 0`. Save/F5 at the first display. Click **Empty kit**, then advance: the next query line must read `Now 0, key false, schedule 0, morning false`. Queries read current canonical values; the already displayed first line stays unchanged. Saving after the UI mutation refuses; Load/F9 restores the packed checkpoint. Advancing again reads two items/three map entries. The reset checkpoint permits saving; quit/relaunch/load and resave preserve its zero state and slot bytes.

For a content-only edit, append `"pass"` to `commands[0].actions[0].value.literal` in `events.jsonc` and bump manifest `saveSchema` to 2. No scalar count action needs updating. Re-export with the same player: output becomes `Packed 3 items`, then `Now 3, key true, schedule 3, morning true`, then zero. Player and compiled dialogue hashes stay identical; old-schema saves refuse safely.

The matching CLI's `check` rejects missing `dialogue.collections/1`, missing/wrong-kind literal collection targets, Session targets in saved Stories and wrong function argument types/counts. Computed keys are checked at runtime; a removed target errors rather than returning zero. Query failure stops before subsequent VM writes and cannot publish a failed restore candidate. See [ADR 0012](../../docs/adr/0012-yarn-collection-queries.md) for exact numeric membership and validation limits. Windows native results remain queued in [issue #5](https://github.com/cunilab/Weftora/issues/5).
