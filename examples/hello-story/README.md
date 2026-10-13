# Hello Story

The Phase 0 walking-skeleton Story. It proves the pipeline end to end: Story folder → `weftora` CLI → prebuilt player → window.

```sh
weftora check examples/hello-story
weftora run examples/hello-story
weftora export examples/hello-story --out dist/hello-story
```

What to expect:

1. A background and a chime, then two lines from the Guide.
2. A choice. "Yes, show me." sets the state key `curious` to `true`; "Not right now." sets it to `false`.
3. The next line reads `curious` and changes accordingly.

Try editing a line in `dialogue/intro.yarn` and running again. The player binary does not change.

Assets (`assets/room.png`, `assets/chime.wav`) are generated placeholders released under CC0.
