---
id: WFT-START-001
title: Create your first Story
status: proposed
---

# Create your first Story

Use a matching `weftora` CLI and `weftora-player` pair from the same build, placed together in one folder. Add that folder to PATH or invoke the CLI by its full path. These are development tools; a supported Windows foundation release is still pending. Story authors do not need Rust to use the tools.

From an existing writable parent folder, run:

```text
weftora new my-story
weftora check my-story
weftora run my-story --headless --choices 0
weftora run my-story
```

`new` creates `manifest.jsonc`, `main.yarn` and a README. The starter has two choices and a boolean variable, uses the built-in dialogue display and needs no external assets. Choose option 1 instead of 0 in the headless command to try the other ending. In the window, advance dialogue with Space and select a choice with its number or the mouse.

Edit the dialogue in `main.yarn`. Set your title and a unique Story `id` in `manifest.jsonc` before distributing it. The starter does not enable saves. For richer content, follow the [Story A tutorial](./story-a-tutorial.md), [presentation guide](./presentation.md) and [checkpoint documentation](./adr/0007-safe-checkpoint-saves.md).

`new` refuses every existing destination, including empty folders, files and links. It does not create missing parent folders. Choose a fresh name; there is no overwrite option. If writing fails after creation, the incomplete new folder remains for inspection. Resolve the reported error and choose a fresh destination, or remove the incomplete folder yourself after inspecting it.

Export to a fresh sibling folder outside the Story:

```text
weftora export my-story --out my-game
```

Run `my-game/weftora-player.exe` on Windows (or the corresponding player executable on a development host). Keep the exported folder together. Re-export after editing the source. For validation without a window, run `my-game/weftora-player.exe my-game/story --validate` from the parent folder.

The CLI finds the player beside itself; `WEFTORA_PLAYER` can select an explicit player path. Export refuses mismatched tool identities before creating output. `weftora --tool-identity` and `weftora-player --tool-identity` show the build and supported compiler/format contract. Install both executables from the same build; there is no force option. Export currently targets the host platform. Automated starter checks and their limits are recorded in [Phase 6 evidence](./evidence/phase-6.md). Native Windows preview and clean-machine acceptance remain tracked in [issue #5](https://github.com/cunilab/Weftora/issues/5).
