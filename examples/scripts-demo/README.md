# Scripts Demo

A Rhai command edits an inventory list and schedule map, derives a scalar count and emits an event. Yarn, buttons and the request-state event call the same command. The Engine contains no kit-specific behavior.

```sh
weftora check examples/scripts-demo
weftora run examples/scripts-demo --headless
weftora export examples/scripts-demo --out /tmp/scripts-preview
/tmp/scripts-preview/weftora-player /tmp/scripts-preview/story
```

Use matching built CLI/player binaries. Headless output is `Packed 2 items`, then `Items after reset 0`. The first checkpoint contains key/ticket and schedule energy 8. At the first display, Save/F5, click **Empty kit**, then **Pack kit**: count returns to 2. Empty again, then **Request pack** exercises state-change event → script → emitted event. Saving after a UI mutation refuses until the next checkpoint; Load/F9 restores the packed checkpoint. **Next checkpoint** resets values and enables saving; quit/relaunch/load must restore zero, and resaving must preserve slot bytes.

Edit only `mechanics.rhai`: change `let limit = amount;` to `let limit = amount + 1;`. Bump manifest `saveSchema` to 2 and re-export using the same player. Fresh output becomes `Packed 3 items`, with key/ticket/pass and energy 7; compiled dialogue and player hashes remain unchanged. An old-schema save must refuse without replacing the slot. Syntax errors, missing functions, denied reads/writes/signals, overflow and exhausted budgets produce Story diagnostics; do not mark those runs as native acceptance without observing them.

See [ADR 0010](../../docs/adr/0010-bounded-rhai-host.md) for permissions and limits. Script-internal Engine commands and seeded randomness are deferred. Native Windows steps remain queued in [issue #5](https://github.com/cunilab/Weftora/issues/5) and do not block development.
