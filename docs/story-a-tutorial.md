---
id: WFT-TUT-001
title: Story A — Edit a Game Without Rust
status: proposed
---

# Story A — Edit a Game Without Rust

This walkthrough changes Last Signal's dialogue, screen/theme and an image using the same player executable. It needs the matching prebuilt CLI and player, a text editor and an image editor or replacement PNG. No Rust toolchain is required. An independent maintainer still needs to run the walkthrough and record P3-07 in [issue #5](https://github.com/cunilab/Weftora/issues/5).

## Get the tools and make a copy

Download weftora-windows-story-a-preview-<sha> from a successful Windows job in the relevant [CI run](https://github.com/cunilab/Weftora/actions/workflows/ci.yml). Match the commit in the implementation PR; a main run from before Story A will not contain it. Artifacts are retained for 14 days and downloading them may require a GitHub login. Extract the complete artifact. It contains:

- baseline: a standalone Story A, its compiled dialogue, player, export report and matching weftora CLI.
- edit-dialogue-screen and edit-theme-image: two content variants and their compiled dialogue, using baseline's identical player.
- working-story: the script's final edited source example.
- same-player-proof.json: actual hashes and validation output for all three exports.

Open PowerShell in the extracted artifact folder. Copy the baseline Story so edits do not change its compiled package:

    Copy-Item -Recurse .\baseline\story .\my-story
    .\baseline\weftora.exe check .\my-story
    .\baseline\weftora.exe run .\my-story

The CLI compiles a temporary dialogue artifact and launches the player beside it. Close the game before the next edit. Do not launch an exported folder after changing its Yarn source without re-exporting: the player deliberately refuses stale compiled dialogue.

Each Story ID has one save slot. The supplied variants deliberately share an ID to test compatibility. Back up an existing Story A save before testing this copy. Keep that ID while checking old saves; choose a new ID in manifest.jsonc when turning the copy into an independent game.

## Change dialogue

In my-story/main.yarn, find Arrival and change Mara's first line:

    Mara: The ferry leaves at dawn. If the beacon stays dark, they will pass us.

Change the ferry sentence, leaving the Mara prefix and the literal checkpoint command above it. Preview again:

    .\baseline\weftora.exe check .\my-story
    .\baseline\weftora.exe run .\my-story

Choose New Game and confirm the new line at Arrival. Keep scene.arrival and saveSchema 1 for this compatible text edit.

Yarn nodes use title, ---, body and ===. Choices begin with -> and their bodies are indented. Variables such as $story__route map to canonical story.route. Declarations set defaults; assignments change state. Random functions and Session reads are refused in checkpoint nodes.

## Change the screen and theme

Open my-story/ui/dialogue.jsonc. The first root child is the LAST SIGNAL title. Change its text or color. The next child is the dialogue panel; change its background, for example from #0B1220F2 to #322033F0. You may also change the choiceList color or size. Preview and confirm both the title and panel changed.

Keep screen IDs, choiceList IDs and action names intact for this first exercise. A screen needs exactly one choiceList. The engine game menu provides Save/Load and Preferences; keep the reading screen focused on dialogue, choices and Continue. See the [UI contract](./ui.md) for supported properties. Structural mistakes are reported with the file and JSON pointer by check.

## Replace an image

The image catalog my-story/assets/images.jsonc maps bg.day to assets/day.png. Replace that PNG with an image of your own, retaining the filename, or update the catalog path to a new file inside my-story.

For a deterministic test without an image editor, copy the included night image over the day image:

    Copy-Item .\my-story\assets\night.png .\my-story\assets\day.png
    .\baseline\weftora.exe run .\my-story

New Game should now use the night-colored image where the day background appeared. Keep media inside the Story. Use a small PNG; images must fit the [decoder and memory bounds](./presentation.md). Replacing the file does not require changing Yarn or the player.

## Save, relaunch and export

On the first Arrival line, wait for the half-second fades to finish, then open the game menu with Escape/right-click and choose Save (or press F5), confirming the operation. Return to the story, continue a few lines, then choose Quit from the game menu and confirm. Preview again and choose Load from the title menu. The Arrival display and Persistent state should return; a baseline Arrival save should also show the compatible edited sentence. Loading preserves current Settings and resets Session. A later line is outside that save point. Load errors appear in the status line without replacing the live game.

Export to a new empty destination:

    .\baseline\weftora.exe export .\my-story --out .\my-game
    .\my-game\weftora-player.exe
    Get-FileHash .\baseline\weftora-player.exe -Algorithm SHA256
    Get-FileHash .\my-game\weftora-player.exe -Algorithm SHA256

Both hashes must match. Copy the entire my-game folder to distribute it. There is no Cargo file or Rust build in this workflow.

## Compare the automated edit packages

These commands run the two supplied content variants on the baseline player:

    .\baseline\weftora-player.exe .\edit-dialogue-screen\story
    .\baseline\weftora-player.exe .\edit-theme-image\story

The first changes the title and Arrival sentence. The second additionally changes the panel theme and day-image bytes. same-player-proof.json records all three actual export hashes before duplicate player copies are removed. The baseline keeps the one executable used by all variants.

Developers can reproduce the packaging proof with existing binaries:

    python scripts/verify-story-a-edits.py --cli <weftora> --player <weftora-player> --out <new-directory>

That script never builds Rust. It checks exports and player validation, then keeps one executable plus both edited content packages. Native visual confirmation and an independent tutorial walkthrough still need maintainer results in issue #5.
