---
id: WFT-UI-001
title: Declarative UI v1
status: proposed
---

# Declarative UI v1

The current visual and interaction direction is documented in [Player interface design](./ui-design.md).

Manifest schema 1 may list `screens` (relative JSONC paths) and an `initialScreen` ID, with the required `ui.screens/1` capability. Each file contains `{"screenSchema":1,"root":{...}}`. The root's ID identifies the screen. Unknown versions and fields fail validation. Without screens, the player uses its original dialogue panel.

Normal launch shows the engine title screen; New Game starts the manifest entrypoint. Existing Yarn title menus still run after that, as Story A demonstrates. Explicit `--start-node` developer previews bypass the title screen and keep save/load disabled. A screen contains exactly one `choiceList`; an overlay contains none. `screen` and `overlay` are root-only. `screen`, `panel`, `list` and `overlay` hold `children`. `text` displays a label, `image` displays a typed image catalog ID, `button` activates one action, and `choiceList` draws the currently offered Yarn choices. See the complete [dialogue screen](../examples/hello-story/ui/dialogue.jsonc) and [overlay](../examples/hello-story/ui/details.jsonc).

Roots, buttons and choice lists require IDs, unique within each file. IDs use lowercase letters, digits, `_`, `.` and `-`, start with a letter or digit, and have at most 64 bytes.

## Layout and appearance

`layout` accepts `position` (`flow` or `absolute`), `left`, `right`, `top`, `bottom`, `width`, `height`, `padding`, `gap`, and `direction` (`column` or `row`). Position/size numbers are pixels; strings ending in `%` are percentages. Padding/gap are pixels. Flow follows the parent container; absolute positions are relative to its container. Image nodes stretch to their specified bounds. Text wraps within available layout width. Rows wrap their children when space runs out and align controls at their vertical center. Player windows have an 800 × 600 logical-pixel minimum.

Text, buttons and choice lists accept `size` (6–200), `color` (`#RRGGBB` or `#RRGGBBAA`) and `font` (a typed font catalog ID). Font-less Story text uses the player's embedded Fira Sans Regular; a configured localisation font takes precedence. Explicit widget fonts still override both. Developer value text keeps Bevy's monospace font. Containers and buttons accept `background` in the same color format. Buttons use at least 44 × 44 logical-pixel targets, rounded corners and borders. Hover and pressed fills derive from the authored background; keyboard navigation adds a separate warm focus border and offset outline. Default widget text is 20 px.

## Bindings and actions

Text and buttons require exactly one of literal `text` or `bind`:

```json
{"type":"text","bind":{"state":"world.day"}}
{"type":"text","bind":{"dialogue":"text"}}
{"type":"text","bind":{"dialogue":"speaker"}}
```

During choices, dialogue text/speaker retain the preceding line when it belongs to the same node and scene context. Node/checkpoint boundaries, background changes and answered choices clear this context; language switching retranslates the retained source line without replaying commands or substitutions. State bindings read canonical Engine state, including Yarn-declared defaults. Booleans display `true`/`false`; integers display decimal; floats use Rust's decimal display; strings display unchanged. Missing speakers are empty. At dialogue end, dialogue text displays the existing end message and the speaker is empty. Labels update in place, preserving their entities and keyboard focus. The adapter snapshots referenced state keys under one read lock, releases it, then patches text. It does not consume state-change notifications.

Buttons require one `action`:

| Action | Fields and behavior |
| --- | --- |
| `dialogue.advance` | Advances a dialogue line; no effect during choices or after end |
| `state.set` | `key`, literal `value`; existing key and exact matching scalar type required |
| `screen.open` | `screen` ID; a screen replaces the base and clears overlays; an overlay opens modally |
| `screen.close` | Only valid inside an overlay; closes the top overlay and restores prior focus |

Yarn numbers map to floats: an action updating a Yarn numeric default supplies `2.0`, not `2`. Actions apply in input order, with at most one dialogue progression per frame. No state lock is held while calling Yarn. Setting state while choices are visible does not re-evaluate that already offered choice set. Opening an already open overlay has no effect; at most four overlays may be open.

## Checkpoint save actions

A save-enabled Story may use button actions {"do":"save"} and {"do":"load"}. They require positive manifest saveSchema and save/1. During play, F5 requests a save and F9 requests a load through the same operations. Valid requests open a confirmation before saving/replacing the single slot or replacing the live session; Cancel is the default focus. Execution revalidates the current checkpoint or save file. In the game menu, use its Save/Load controls instead of these shortcuts. Saving is allowed at the first display after a checkpoint, on the initial base screen with all overlays closed, after view/audio fades finish. Playing music or a one-shot does not itself block saving.

Save/load requests run after the frame's other inputs and presentation commands. Progression or presentation commands in that frame refuse the request; combined F5/F9 and keyboard progression does not advance the restored display. Load restores the initial base and resets choice focus. Current Settings remain; Session resets. Save validates all screens' bindings and current volumes before replacing the slot, including unopened overlays, so it can refuse data that would not load. File/compatibility errors are nonfatal and appear in an adapter-owned backed notification above Story UI. Successful save/load notices clear after four seconds, including after repeating the same operation. Refusals stay visible and identify save or load. Notification backgrounds block click-through. Idle shortcut instructions no longer cover the scene.

The status may include an absolute user-data path when reporting an IO error. Save/load performs bounded synchronous file operations; flush and replacement can briefly pause a frame. Load may be requested with an overlay open and returns to the initial base. A Save button inside a pause overlay will refuse until the overlay closes. Loading after a terminal Story error is unavailable. Checkpoint openings must reconstruct the full scene and intended audio; playback positions and UI overlays are not stored. See [ADR 0007](./adr/0007-safe-checkpoint-saves.md).

## Player lifecycle

The engine title and game menu are separate from authored screen layers and require no Story/schema changes. Opening the menu neither executes Yarn nor changes checkpoint validity. Existing fades and music continue; Story progression is suspended. Game-menu Save shows its eligibility or refusal reason before activation, while storage errors can still occur when executing a request. Load availability is refreshed on menu entry and after operations, with the file revalidated at execution. One slot remains one slot: no file browser or rollback is implied.

New Game builds fresh Story state while retaining current Settings. Main Menu clears the scene only after confirmation. Load candidates remain isolated until dialogue, bindings and media validate, so failure leaves the current session intact. Preferences across application launches, read-only history and migration of authored title menus are follow-up work in the [UX plan](./renpy-ux-review.md); the title currently offers New Game, Load checkpoint, Controls and Quit.

## Input and failures

Tab/Shift+Tab navigate the active focus group. Enter/Space activate its focused button. Clicking a button activates its action; clicking the unoccupied background advances a line. Gaps in button toolbars block click-through. Authored Advance buttons are hidden during choices and at the end, and removed from keyboard focus until a line returns. Digits 1–9 choose visible choices when no overlay is open. Overlays block underlying button and background input. When keyboard navigation is active, new choices focus the first choice; closing an overlay restores the previous focus by screen/widget ID. Escape/right-click closes the top authored overlay first; otherwise it opens or closes the engine game menu. Menu and confirmation input cannot progress the Story behind them. Return restores the same display and keyboard focus; missing menu focus recovers to an enabled control. Repeated Escape never quits. Quit, window-close requests and Main Menu use explicit progress-loss confirmations. A window-close request also closes the developer drawer so the confirmation is visible.

Story errors use an adapter-owned error panel above every Story screen, with Story input ignored; the engine menu remains available for returning to the title or quitting. Save and Load are unavailable after a terminal Story error. Image/font paths use Story confinement and bounded reads, then decoder validation; no Story path is passed to AssetServer. `check`, preview and export share structural/reference/state-action validation before activation. Font decoding and graphics remain renderer work.

Limits: 64 screen files, 256 KiB/file, 512 widgets/screen, depth 16, 128 children/container, 4096 bytes/literal or bound text, finite pixel lengths 0–8192 and percentages 0–100. Asset files retain the Story package's existing size limits. Images are limited to 4096 pixels per dimension and a conservative 64 MiB decode estimate; all preloaded UI images/fonts share that 64 MiB estimate. This permits about 4 megapixels per image and in total across UI images, with less available when fonts are included; a 4K image exceeds the estimate. This bounds asset data, not the whole engine's CPU/GPU memory. Author errors include the screen filename and JSON pointer; structural errors do not include line numbers. Images draw above the Yarn background layer, and overlays support both their authored close buttons and Esc.

Windows rendering, focus, resize and font behavior still need the maintainer's verification in [issue #5](https://github.com/cunilab/Weftora/issues/5) and the [blank test record](./evidence/phase-2-windows.md). Automated widget tests do not prove native rendering or performance.

## Story command actions

A button can use `{"do":"command.call","command":"award","args":[3]}`. The loader checks the named command and canonical scalar arguments against the Story's `rules.commands/1` definitions. [Typed commands](./rules.md#typed-declarative-commands) share the rule bridge with UI `state.set` actions; both settle new state notifications before bindings refresh. Persistent UI mutations invalidate the current checkpoint save point until the next checkpoint. Existing modal/origin input gating applies before any command executes.

## Custom screen walkthrough

The [Station Desk demo](../examples/screens-demo/README.md) provides three replaceable base screens and a pause overlay. Layout/theme/navigation are authored directly in screen JSONC; there is no separate theme registry. Opening another base keeps the current Yarn display and canonical state, clears overlays and resets active keyboard focus to that screen. Closing an overlay restores its initiating focus key. A base screen still requires exactly one choice list, including a menu or activity view, because any view may show the current Yarn choices.

Run `scripts/verify-screen-edits.py` with prebuilt CLI/player paths to export baseline and UI-only edits. It checks player/compiled-dialogue hashes and validates the edited package with the baseline player. Renderer model tests check navigation, focus, input gating and authored Bevy layout/color components. CLI fixtures check malformed widgets, bindings, colors, targets and state-action types. These automated checks do not prove native drawing, pointer picking or readable resizing; those steps remain in issue #5.
