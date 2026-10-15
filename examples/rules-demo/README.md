# Rules Demo

A small content-defined rule awards ten points when Yarn sets `input` to true. No Rust edit is needed.

```sh
weftora check examples/rules-demo
weftora run examples/rules-demo --headless
weftora run examples/rules-demo --headless --save-at rules.start --save-dir /tmp/weftora-rules-saves
weftora run examples/rules-demo --headless --load --save-dir /tmp/weftora-rules-saves
```

On Windows, choose an external writable folder for `--save-dir`, such as `$env:TEMP\weftora-rules-saves`. Without an override, saves use the normal per-user directory.

The first line says `First points 0`: interpolation occurred before rules settled at that dialogue boundary. The following line says `After rules 10`. Both the initial display state and a loaded checkpoint have ten points and `awarded: true`; loading does not award twice. Changing the action amount in `events.jsonc` changes the mechanic without changing the player. Such a mechanics change should also bump `saveSchema` before distributing content updates.

The ordinary `weftora run` command uses the same Story-aware runner in the player. Rules from external UI writes are not part of this initial bridge. Native rendering/input/save-relaunch checks remain queued in [issue #5](https://github.com/cunilab/Weftora/issues/5).

See the [rules schema and execution contract](../../docs/rules.md) and [ADR 0008](../../docs/adr/0008-declarative-events.md).
