---
id: WFT-UI-001
title: Declarative UI v1
status: proposed
---

# Declarative UI v1

Manifest schema 1 may list `screens` (relative JSONC paths) and an `initialScreen` ID, with the required `ui.screens/1` capability. Each file contains `{"screenSchema":1,"root":{...}}`. The root's ID identifies the screen. Unknown versions and fields fail validation. Without screens, the player uses its original dialogue panel.

Screens frame dialogue that starts at launch. A title menu can be a Yarn choice node on the initial screen, as Story A demonstrates. A screen contains exactly one `choiceList`; an overlay contains none. `screen` and `overlay` are root-only. `screen`, `panel`, `list` and `overlay` hold `children`. `text` displays a label, `image` displays a typed image catalog ID, `button` activates one action, and `choiceList` draws the currently offered Yarn choices. See the complete [dialogue screen](../examples/hello-story/ui/dialogue.jsonc) and [overlay](../examples/hello-story/ui/details.jsonc).

Roots, buttons and choice lists require IDs, unique within each file. IDs use lowercase letters, digits, `_`, `.` and `-`, start with a letter or digit, and have at most 64 bytes.

## Layout and appearance

`layout` accepts `position` (`flow` or `absolute`), `left`, `right`, `top`, `bottom`, `width`, `height`, `padding`, `gap`, and `direction` (`column` or `row`). Position/size numbers are pixels; strings ending in `%` are percentages. Padding/gap are pixels. Flow follows the parent container; absolute positions are relative to its container. Image nodes stretch to their specified bounds. Text wraps within available layout width.

Text, buttons and choice lists accept `size` (6–200), `color` (`#RRGGBB` or `#RRGGBBAA`) and `font` (a typed font catalog ID). Font-less text uses Bevy's bundled font. Containers and buttons accept `background` in the same color format. Buttons have adapter-owned hover/focus colors.

## Bindings and actions

Text and buttons require exactly one of literal `text` or `bind`:

```json
{"type":"text","bind":{"state":"world.day"}}
{"type":"text","bind":{"dialogue":"text"}}
{"type":"text","bind":{"dialogue":"speaker"}}
```

State bindings read canonical Engine state, including Yarn-declared defaults. Booleans display `true`/`false`; integers display decimal; floats use Rust's decimal display; strings display unchanged. Missing speakers are empty. At dialogue end, dialogue text displays the existing end message and the speaker is empty. Labels update in place, preserving their entities and keyboard focus. The adapter snapshots referenced state keys under one read lock, releases it, then patches text. It does not consume state-change notifications.

Buttons require one `action`:

| Action | Fields and behavior |
| --- | --- |
| `dialogue.advance` | Advances a dialogue line; no effect during choices or after end |
| `state.set` | `key`, literal `value`; existing key and exact matching scalar type required |
| `screen.open` | `screen` ID; a screen replaces the base and clears overlays; an overlay opens modally |
| `screen.close` | Only valid inside an overlay; closes the top overlay and restores prior focus |

Yarn numbers map to floats: an action updating a Yarn numeric default supplies `2.0`, not `2`. Actions apply in input order, with at most one dialogue progression per frame. No state lock is held while calling Yarn. Setting state while choices are visible does not re-evaluate that already offered choice set. Opening an already open overlay has no effect; at most four overlays may be open.

## Checkpoint save actions

A save-enabled Story may use button actions {"do":"save"} and {"do":"load"}. They require positive manifest saveSchema and save/1. F5 saves and F9 loads through the same operations. Saving is allowed at the first display after a checkpoint, on the initial base screen with all overlays closed, after view/audio fades finish. Playing music or a one-shot does not itself block saving.

Save/load requests run after the frame's other inputs and presentation commands. Progression or presentation commands in that frame refuse the request; combined F5/F9 and keyboard progression does not advance the restored display. Load restores the initial base and resets choice focus. Current Settings remain; Session resets. Save validates all screens' bindings and current volumes before replacing the slot, including unopened overlays, so it can refuse data that would not load. File/compatibility errors are nonfatal and appear in an adapter-owned status line above Story UI.

The status may include an absolute user-data path when reporting an IO error. Save/load performs bounded synchronous file operations; flush and replacement can briefly pause a frame. Load may be requested with an overlay open and returns to the initial base. A Save button inside a pause overlay will refuse until the overlay closes. Loading after a terminal Story error is unavailable. Checkpoint openings must reconstruct the full scene and intended audio; playback positions and UI overlays are not stored. See [ADR 0007](./adr/0007-safe-checkpoint-saves.md).

## Input and failures

Tab/Shift+Tab navigate the active focus group. Enter/Space activate its focused button. Clicking a button activates its action; clicking the unoccupied background advances a line. Digits 1–9 choose visible choices when no overlay is open. Overlays block underlying button and background input. When keyboard navigation is active, new choices focus the first choice; closing an overlay restores the previous focus by screen/widget ID. Esc quits.

Story errors use an adapter-owned error panel above every Story screen, with input ignored except Esc. Image/font paths use Story confinement and bounded reads, then decoder validation; no Story path is passed to AssetServer. `check`, preview and export share structural/reference/state-action validation before activation. Font decoding and graphics remain renderer work.

Limits: 64 screen files, 256 KiB/file, 512 widgets/screen, depth 16, 128 children/container, 4096 bytes/literal or bound text, finite pixel lengths 0–8192 and percentages 0–100. Asset files retain the Story package's existing size limits. Images are limited to 4096 pixels per dimension and a conservative 64 MiB decode estimate; all preloaded UI images/fonts share that 64 MiB estimate. This permits about 4 megapixels per image and in total across UI images, with less available when fonts are included; a 4K image exceeds the estimate. This bounds asset data, not the whole engine's CPU/GPU memory. Author errors include the screen filename and JSON pointer; structural errors do not include line numbers. Images draw above the Yarn background layer, and an overlay needs its own close button to return without quitting.

Windows rendering, focus, resize and font behavior still need the maintainer's verification in [issue #5](https://github.com/cunilab/Weftora/issues/5) and the [blank test record](./evidence/phase-2-windows.md). Automated widget tests do not prove native rendering or performance.
