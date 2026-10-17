# Commands Demo

A small Story calls the same typed `award` command from Yarn, an event, and a screen button. No Rust changes are needed to edit its mechanics. See [the rule contract](../../docs/rules.md#typed-declarative-commands).

```sh
cargo run -p weftora-cli -- check examples/commands-demo
cargo run -p weftora-cli -- run examples/commands-demo --headless
```

The headless lines are `First points 2` and `Next points 3`. `award` calls the later-defined `increment`; event and command IDs may both be `award`.

For the graphical demo, use a matching built player and CLI:

```sh
weftora export examples/commands-demo --out /tmp/commands-preview
/tmp/commands-preview/weftora-player /tmp/commands-preview/story
```

1. At `First points 2`, Save (or F5). The points binding shows 2.
2. Click **Award 3**: the binding shows 5. Saving now refuses and preserves the previous slot because this UI input is absent from checkpoint replay.
3. Load (or F9): the first line and binding return to 2.
4. Click **Set input**: its `state.set` notification triggers the event, which calls `award 10`. The binding shows 12. Clicking again is a no-op and stays at 12.
5. Click **Next checkpoint**: the next Yarn command adds 1, showing `Next points 13`. Save now succeeds; loading reproduces 13.

The first dialogue line stays at its original interpolated value while the state binding updates after button actions. Persistent UI mutations require a new checkpoint before saving; loading the previous save discards them.

To edit mechanics, change the event argument 10 or the button argument 3 in JSONC, bump `saveSchema` in the manifest, and re-export with the same player. The exported dialogue is validated against the current rule definitions. Keep the matching CLI for re-export; do not hand-edit exported metadata/artifacts. Native Windows verification remains queued in [issue #5](https://github.com/cunilab/Weftora/issues/5).
