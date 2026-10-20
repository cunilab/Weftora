# My Story

Edit `main.yarn` to change the dialogue and choices. Set the title and a unique
`id` in `manifest.jsonc` before distributing your Story. The starter uses the
built-in dialogue screen and needs no images, audio, scripts or Rust code.

With prebuilt `weftora` and `weftora-player` available, run these commands from
this folder:

```text
weftora check .
weftora run . --headless --choices 0
weftora run .
weftora export . --out ../my-game
```

Keep the matching player beside the CLI, or set `WEFTORA_PLAYER` to its path.
Use a fresh export directory outside this Story. The exported game contains
its own player and compiled dialogue; it needs no Rust or Cargo to launch.

The template has no save support. Add the save contract and checkpoints when
your Story needs them. To create another project, use `weftora new <new-folder>`
with an existing parent. Existing folders are always refused, even empty ones.
If writing fails, an incomplete new folder is kept for inspection.
