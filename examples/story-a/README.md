# Last Signal — Story A

A short, complete visual novel about Mara and Ivo sending a message before the morning ferry leaves. Begin leads to two routes: repair the receiver with Ivo, or carry the battery to the roof with Mara. Both reach a distinct reply at dawn. About returns to the title menu.

From the repository, use a matching prebuilt CLI/player or development binaries:

    weftora check examples/story-a
    weftora run examples/story-a

No Rust is inside this Story. Hello Story remains the original golden-test fixture; Story A is separate so those earlier traces remain stable.

Continue, Space or a background click advances a line. Digits 1–2 choose; Tab/Shift+Tab and Enter navigate buttons. Volume opens the channel controls; close it before saving. Save/Load buttons and F5/F9 use one slot for this Story ID. Saving works only at the first line or choices of a checkpoint, after fades finish. A refused operation appears in the top status line and leaves the game active.

Checkpoint IDs: scene.menu, scene.arrival, scene.decision, scene.repair, scene.roof and scene.dawn. At Repair/Roof, the opening increments story.attempts once; saving and reloading that display must still show one attempt. The route and sent-signal bindings show canonical state. Compatible dialogue/image edits retain saveSchema 1 and these IDs.

Headless branch and checkpoint examples:

    weftora run examples/story-a --headless --choices 0,0
    weftora run examples/story-a --headless --choices 0,1
    weftora run examples/story-a --headless --choices 0 --save-at scene.decision --save-dir <external-test-directory>
    weftora run examples/story-a --headless --load --choices 1 --save-dir <external-test-directory>

The images and quiet one-second sine tones are the CC0 prototype fixtures from presentation-demo. Rectangles stand in for characters/CG; this example demonstrates the pipeline and narrative flow, with no claim of finished illustration or music.

See the [author tutorial](../../docs/story-a-tutorial.md), [Windows steps and blank results](../../docs/evidence/phase-3-windows.md) and [issue #5](https://github.com/cunilab/Weftora/issues/5). Native results and an independent author's tutorial walkthrough remain pending.
