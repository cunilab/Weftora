# Collections Demo

A kit inventory (`items`) and schedule (`schedule`) live in canonical list/map state. The `pack` and `unpack` Story commands change whole values; Yarn and screen bindings read the separately authored scalar `item_count` and `status` fields. No Engine-specific inventory behavior is needed.

```sh
weftora check examples/collections-demo
weftora run examples/collections-demo --headless
weftora export examples/collections-demo --out /tmp/collections-preview
/tmp/collections-preview/weftora-player /tmp/collections-preview/story
```

Use a matching built player and CLI. Headless dialogue shows `Packed 2 items`, then `Items after reset 0`. The map deliberately contains an `argument` key; its explicit `{"literal": {...}}` action wrapper keeps it separate from command argument references.

At the first display, Save/F5, click **Empty kit**, and confirm the status/count bindings become `Empty kit` and 0. Saving refuses until the next checkpoint because UI mutations are absent from save replay. Load/F9 restores `Kit packed` and 2. **Pack kit** is a no-op when already packed. **Next checkpoint** resets the collections and permits saving the next display; quit/relaunch/load must restore that display with empty values, without changing saved slot bytes on resave.

To edit the inventory, add `"pass"` to the pack command's list and change its authored `item_count` action from 2 to 3. Bump manifest `saveSchema` to 2 and re-export with the same player. A fresh run shows `Packed 3 items`; the player and compiled dialogue hashes remain unchanged. Old schema-1 saves must refuse safely.

Collections are flat and bounded; nested arrays/maps, null children and collection-valued Yarn variables/text bindings refuse. Whole-value set/reset/remove work; append, entry updates, collection queries and Rhai are future work. See [the collection contract](../../docs/adr/0009-flat-collection-state.md). Native Windows results remain queued in [issue #5](https://github.com/cunilab/Weftora/issues/5).
