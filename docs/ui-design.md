---
id: WFT-UX-001
title: Player interface design
status: proposed
---

# Player interface design

The visual pass and first engine title/game-menu slice are implemented; overall player UX remains unaccepted. The [Ren'Py workflow review](./renpy-ux-review.md) identifies the missing title/game-menu lifecycle, decision context and progress-safety flows, and defines the next behavioral acceptance checks. The implemented lifecycle and its remaining limits are described in [Declarative UI](./ui.md#player-lifecycle).

The player uses a modern game interface: deep navy surfaces, warm gold accents, proportional dialogue text and bordered controls. The story remains the visual focus. The examples share these defaults while keeping their authored layouts, state bindings and navigation.

## Reading and interaction

- Dialogue uses Fira Sans Regular with a clear speaker/body hierarchy. Explicit Story fonts and the localisation font override it; the English/Indonesian/Japanese demo keeps Noto Sans JP.
- Buttons have 44-pixel minimum targets, rounded borders, authored-color hover/pressed fills and a separate keyboard focus border and offset outline. Toolbar gaps cannot advance dialogue accidentally.
- Continue appears only when a line can advance. Choices retain their IDs, order and numeric shortcuts, plus relevant preceding dialogue context within the same node/scene. Switching languages preserves the current offer and keyboard focus.
- Rows wrap at smaller widths. The player has an 800 × 600 logical-pixel minimum; authored absolute layouts still need testing with their actual content.
- Escape/right-click closes the top Story overlay or opens/closes the game menu. Return restores the current offer and focus. Quit, Main Menu and operating-system window-close requests use explicit confirmations; repeated Escape never quits.
- Save/load feedback has its own backed notification: green successes clear after four seconds; amber refusals name the failed action and remain until the next request. Notifications block accidental click-through. Permanent shortcut instructions no longer cover the scene. Checkpoint requirements and save-file validation remain the existing contract.

## Author tools

F12 opens a right-side developer drawer, keeping the scene visible behind a dimmed input blocker. State, Commands and Event trace are clickable tabs; Tab retains keyboard tool switching. JSON values stay monospace. Long content scrolls with the mouse wheel/trackpad; Home and End jump to the top or bottom. The existing bounded editor, pagination, command validation and save protections remain in force.

This pass changes the interface rather than Story artwork. Existing colored character and scene assets remain intentional prototype content. The renderer stays on Bevy UI, with no new UI dependency or Story/save schema.

## Review

Automated renderer tests cover dormant startup, menu suspend/resume, focus recovery, confirmations, overlay dismissal, unavailable Continue focus, tool-tab switching and existing save/modal/localisation behavior. The embedded Fira Sans bytes and release notice are pinned and checked. Rendering captures and platform limits are recorded in [Phase 6 evidence](./evidence/phase-6.md#player-ui-and-interaction-redesign).

Windows pointer picking, keyboard feel, resizing, DPI and readability remain queued in [issue #5](https://github.com/cunilab/Weftora/issues/5). Native acceptance must be checked again against the redesigned player; older screenshots describe the old interface.
