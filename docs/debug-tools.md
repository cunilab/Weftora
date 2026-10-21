---
id: WFT-DBG-001
title: Developer tools
status: proposed
---

# Developer tools

## Enable the state inspector

Start an author preview explicitly in developer mode:

```text
weftora run my-story --debug-tools
```

For an already exported game, use `weftora-player.exe my-game/story --debug-tools`. From the game's folder, `weftora-player.exe --debug-tools` uses the adjacent `story/` folder. An explicit dialogue artifact may be selected before the flag: `weftora-player.exe my-story --dialogue compiled/dialogue.json --debug-tools`.

The window title identifies developer mode. Press **F12** to open the inspector. Normal launches keep the inspector disabled. The option is available in release builds so authors can verify the actual release player. It is an explicit development mode, not a Story capability or a save-format change.

## Inspect and edit

| Control | Action |
| --- | --- |
| Tab | Switch between state and command mode |
| Up / Down | Select a state key in state mode |
| PageUp / PageDown | Move by twelve keys |
| Ctrl+A | Clear the current JSON input for replacement |
| Backspace | Delete the last Unicode character |
| Enter | Apply the state value or execute the command |
| F12 / Escape | Close the inspector |

Keys appear in sorted order with their persistence scope and current value. Removed declarations remain visible as `<removed>`; this first editor refuses writes to removed or unknown keys. Values and keys have shortened previews; the selected key appears in full. The editor retains at most 16 KiB of JSON and displays the last 240 characters of input. It provides append/backspace editing; caret movement, clipboard paste and undo are outside this first slice.

Use JSON values such as `true`, `42`, `0.5`, `"new text"`, `[1,2]` or `{"permit":true}`. The existing key type remains fixed; a float key needs a JSON float such as `1.0`. Canonical collection/value bounds still apply. Invalid JSON, unknown keys, type mismatches and incompatible dialogue/display/audio values are rejected before touching the runner. Rejections appear inside the inspector and preserve the live state and current checkpoint.

Accepted edits pass through the normal rule/event bridge. Derived rule changes settle as they do for a Story UI action. Actual Story rule/script failures retain the runtime's terminal error behavior and may retain completed rule writes. Inspecting/editing does not reconstruct dialogue or repeat prior commands/RNG. The displayed line and offered choices remain their existing snapshots until normal Story progression.

## Run a Story command

Press **Tab** in the open panel to switch to the command runner. Enter the same JSON action used by a Story screen, for example in Commands Demo:

```json
{"do":"command.call","command":"award","args":[10]}
```

Only commands declared by the Story's rules are available. Names and parameter types come from the Story's command definitions; there is no command catalog, history or autocomplete in this first slice. Presentation, checkpoint, save/load, screen and node actions are unavailable. The existing strict action parser rejects unknown/duplicate fields; omitted `args` means `[]`. Integers and floats remain distinct.

Malformed JSON, unknown commands, wrong argument counts/types and value-limit violations are rejected before execution, preserving state and checkpoint eligibility. A valid command runs once through the ordinary rules/script bridge without a dry run or dialogue replay. Actual command failures (such as state arithmetic overflow or a script error) retain completed writes and put the Story into its normal terminal error state. Commands can also change values that make a later dialogue/display validation fail; parameter validation does not predict command effects.

The command buffer remains when changing modes or closing the panel. **Each new Enter press runs it again**; holding Enter does not resubmit. Tab consumes its frame, including a simultaneous Enter. Returning to state mode reloads the selected key's current value. Commands have the same input bound, capture, persistent-save invalidation and snapshot-dialogue behavior as state edits.

## Input and saves

While the inspector is open, Story keyboard/pointer intents and save/load shortcuts are captured. The opening and closing frames are captured too: Escape closes the inspector, and Enter applies an edit without also advancing dialogue. Existing presentation actions and media continue processing. Closing restores the previous focus when that entity still exists.

A persistent edit invalidates the current checkpoint, including persistent changes caused by rules. Close the inspector and reach a new authored checkpoint before saving. Session/settings edits retain save eligibility when they do not cause persistent changes. Loading a valid save replaces canonical state; the next inspector opening reads that live replacement. The inspector never writes a Story source or save file directly.

## Remaining tools and acceptance

Signal/event tracing and starting at a selected Yarn node remain separate P6-10 work. The [Phase 6 evidence](./evidence/phase-6.md#developer-state-inspector-p6-10-partial) records automated coverage. Native Windows text entry, focus/pointer capture, resize/DPI and save/load walkthroughs remain in [issue #5](https://github.com/cunilab/Weftora/issues/5); this slice does not close the full debug-tools or foundation-release gate.

## Windows acceptance checklist

Use the matching release tools from a successful CI run for this PR. Copy `examples/commands-demo` into a fresh writable folder for the rule/save cases. Give the copied manifest a unique test Story ID to isolate its save directory, and keep a backup of any test saves. These steps remain maintainer-owned in issue #5.

- [ ] Normal preview: F12 opens no inspector. With `--debug-tools`, the title identifies developer mode and F12 opens the panel; sorted keys/scopes and the selected full key are readable.
- [ ] While open, Space, digits, Story-button clicks and F5/F9 do not advance, mutate, save or load. F12/Escape closes without quitting or progressing on that frame. Keyboard focus returns appropriately; pending media/transitions continue.
- [ ] On Commands Demo's first line, save before editing. Reject a string for integer `points` and malformed JSON without changing values or save eligibility. Set Boolean `input` to `true`: rules add ten points and set `awarded`; displayed dialogue stays at its previous line until normal progression.
- [ ] Close and try saving after that persistent edit: refusal preserves the existing slot/backup. F9 loads the old save; reopening reads restored values. Repeat the edit, reach the next authored checkpoint and verify save/load there works without repeating rewards.
- [ ] In a copied Story with declared string/session and collection keys, check Unicode/AltGr input, UTF-8 backspace, Ctrl+A, valid JSON types, the 16 KiB limit, and rejection of nested/oversized collections. Session-only changes retain save eligibility when rules do not change persistent state.
- [ ] Switch to command mode with Tab. Reject malformed JSON, `save`, an unknown command, and `award` with a string argument without mutation or save invalidation. Execute `{"do":"command.call","command":"award","args":[10]}`: exactly ten points and `awarded=true`, unchanged displayed line, and save refused until a new checkpoint. Hold Enter (no repeat), release and press again (another ten points); Tab+Enter changes mode without execution. Close/reopen and verify the command buffer remains.
- [ ] Check pointer capture, focus loss/resume, keyboard navigation, resize and supported DPI settings. Inspect long keys/values and multiple pages for clipping/readability. Record OS, GPU, layout and observed issues before signing off.
