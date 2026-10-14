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
