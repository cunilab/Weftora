# Hello Story

The Phase 0 walking-skeleton Story. It proves the pipeline end to end: Story folder → `weftora` CLI → prebuilt player → window.

```sh
weftora check examples/hello-story
weftora run examples/hello-story
weftora run examples/hello-story --headless --choices 0
weftora export examples/hello-story --out dist/hello-story
```

What to expect:

1. A background and a chime, then two lines from the Guide.
2. A choice. "Yes, show me." sets the state key `curious` to `true`; "Not right now." sets it to `false`.
3. The next line reads `curious` and changes accordingly.

Headless mode emits deterministic JSON-lines commands, dialogue, choices, final state, changes, observed state-change signals and the actual Story-service lifecycle. It validates through the same loader as `check`, consumes the supplied zero-based option IDs, and does not open a window or play media. Use `--choices 1` for the other branch. Missing, invalid or unused choices fail with a non-zero exit. Output is streamed; a runtime error can leave a partial trace, so always check the exit code.

Try editing a line in `dialogue/intro.yarn` and running again. The player binary does not change.

Assets (`assets/room.png`, `assets/chime.wav`) are generated placeholders released under CC0.

The image and audio IDs are declared in `assets/images.jsonc` and `assets/audio.jsonc` under manifest `catalogs`. Legacy `assets: {id: path}` still loads, but has no declared media type. Catalog IDs share one namespace; image/audio/font category mismatches are rejected before gameplay.

The Phase 2 UI lives in `ui/dialogue.jsonc` and `ui/details.jsonc`. Next advances a line; Details opens a modal overlay whose state button writes canonical `curious`. Closing returns keyboard focus to Details. Tab/Shift+Tab navigate buttons, Enter/Space activate, and Esc closes an overlay; otherwise press Esc twice within three seconds to quit. See [UI v1](../../docs/ui.md) for the schema. Headless mode validates screens but does not execute UI actions. Native Windows checks remain in [issue #5](https://github.com/cunilab/Weftora/issues/5).
