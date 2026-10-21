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
| Tab | Cycle state → command → trace → state |
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

Press **Tab** from state mode to switch to the command runner. Enter the same JSON action used by a Story screen, for example in Commands Demo:

```json
{"do":"command.call","command":"award","args":[10]}
```

Only commands declared by the Story's rules are available. Names and parameter types come from the Story's command definitions; there is no command catalog, history or autocomplete in this first slice. Presentation, checkpoint, save/load, screen and node actions are unavailable. The existing strict action parser rejects unknown/duplicate fields; omitted `args` means `[]`. Integers and floats remain distinct.

Malformed JSON, unknown commands, wrong argument counts/types and value-limit violations are rejected before execution, preserving state and checkpoint eligibility. A valid command runs once through the ordinary rules/script bridge without a dry run or dialogue replay. Actual command failures (such as state arithmetic overflow or a script error) retain completed writes and put the Story into its normal terminal error state. Commands can also change values that make a later dialogue/display validation fail; parameter validation does not predict command effects.

The command buffer remains when changing modes or closing the panel. **Each new Enter press runs it again**; holding Enter does not resubmit. Tab consumes its frame, including a simultaneous Enter. Tab cycles through the read-only trace view before returning to state mode. Returning to state mode reloads the selected key's current value. Commands have the same input bound, capture, persistent-save invalidation and snapshot-dialogue behavior as state edits.

## Trace signals and events

Press **Tab** twice from state mode to open the read-only trace view. Developer launches capture scheduler traces from before the first displayed line, even while the panel is closed. Normal launches retain no trace history. The latest twelve rows are displayed; Up/Down scroll by one row and PageUp/PageDown by twelve. Returning to the newest page follows the tail. Delete clears history and its dropped-row count. Enter and printable input do nothing in trace mode; the command buffer is preserved.

Rows show delivered signals with bounded debug-value previews, event conditions as `matched` or `skipped`, zero-based indexes of completed actions, and scheduler errors after any completed rows. Signals emitted by actions/scripts and subscribed `state.changed` notifications appear in their existing dispatch order. Notifications without a matching scheduler subscription are not dispatched and do not appear. This is a rules-scheduler trace, not a general log of every state write, Engine signal bus, command attempt or script instruction. Parse/preflight failures remain command-editor messages; failures before scheduler dispatch have no trace rows.

History keeps the latest **128 rows**, each limited to **256 Unicode characters**, with a count of older rows dropped. Control characters are escaped and truncated rows end in `…`; previews can split grapheme clusters. Trace rows do not wrap and are clipped at the panel edge; narrower windows may hide the end of a row. This first view has no search, export or persistence.

Tracing only observes existing dispatch results; it does not rerun commands, evaluate extra conditions, consume dispatch/RNG budgets, edit state or invalidate checkpoints. A successful load starts fresh empty history after candidate replay; replay rows are intentionally excluded. A failed load retains the live runner's history. Cloned rule definitions never inherit tracing. Actual runtime failures leave completed rows available for inspection; the normal Story error behavior still applies.

## Start at a Yarn node

Select an exact, case-sensitive compiled Yarn node when launching a fresh developer session:

```text
weftora run my-story --debug-tools --start-node Later
weftora-player.exe my-game/story --debug-tools --start-node Later
weftora-player.exe --debug-tools --start-node Later
```

The third command uses the adjacent `story/` folder. An explicit artifact goes before the developer options: `weftora-player.exe my-story --dialogue compiled/dialogue.json --debug-tools --start-node Later`. Keep this option order. `--start-node` requires `--debug-tools` and cannot be combined with `--headless` or `--validate`. Unknown, empty and case-mismatched names fail before a player/window starts. Use the exact `title:` from the authored Yarn source; compiler-generated node groups/helpers are not a supported authoring feature of the pinned compiler.

Each launch starts from Story/Yarn defaults (Persistent, Session and Settings), fresh visit/script state and the authored RNG seed. Earlier nodes are not replayed. The selected node runs its commands once to the first line/choice using ordinary rule, budget and presentation behavior; trace capture is already enabled. Content that assumes earlier state may fail or show different results. There is no in-session node picker or jump, and the manifest/compiled artifact are not rewritten.

The title identifies the selected node and **save/load disabled**. Both F5/F9 and authored save/load buttons refuse for the entire `--start-node` session, including after reaching an authored checkpoint; the normal slot/backup is never read or written. Relaunch without `--start-node` for normal save/load testing. Runtime checkpoint eligibility still follows authored checkpoints; selection creates no synthetic checkpoint. Visit counts follow the pinned runtime's normal semantics: tracked visits increment when leaving a node, so the first display in a freshly selected node sees zero prior visits. Fresh sessions do not inherit visits from another process.

## Input and saves

While the inspector is open, Story keyboard/pointer intents and save/load shortcuts are captured. The opening and closing frames are captured too: Escape closes the inspector, and Enter applies an edit without also advancing dialogue. Existing presentation actions and media continue processing. Closing restores the previous focus when that entity still exists.

A persistent edit invalidates the current checkpoint, including persistent changes caused by rules. Close the inspector and reach a new authored checkpoint before saving. Session/settings edits retain save eligibility when they do not cause persistent changes. Loading a valid save replaces canonical state; the next inspector opening reads that live replacement. The inspector never writes a Story source or save file directly.

## Remaining tools and acceptance

State editing, command execution, rules tracing and launch-time node selection are implemented; full native release-build acceptance remains open. The [Phase 6 evidence](./evidence/phase-6.md#developer-state-inspector-p6-10-partial) records automated coverage. Native Windows text entry, focus/pointer capture, resize/DPI and save/load walkthroughs remain in [issue #5](https://github.com/cunilab/Weftora/issues/5); this slice does not close the full debug-tools or foundation-release gate.

## Windows acceptance checklist

Use the matching release tools from a successful CI run for this PR. Copy `examples/commands-demo` into a fresh writable folder for the rule/save cases. Give the copied manifest a unique test Story ID to isolate its save directory, and keep a backup of any test saves. These steps remain maintainer-owned in issue #5.

- [ ] Normal preview: F12 opens no inspector. With `--debug-tools`, the title identifies developer mode and F12 opens the panel; sorted keys/scopes and the selected full key are readable.
- [ ] While open, Space, digits, Story-button clicks and F5/F9 do not advance, mutate, save or load. F12/Escape closes without quitting or progressing on that frame. Keyboard focus returns appropriately; pending media/transitions continue.
- [ ] On Commands Demo's first line, save before editing. Reject a string for integer `points` and malformed JSON without changing values or save eligibility. Set Boolean `input` to `true`: rules add ten points and set `awarded`; displayed dialogue stays at its previous line until normal progression.
- [ ] Close and try saving after that persistent edit: refusal preserves the existing slot/backup. F9 loads the old save; reopening reads restored values. Repeat the edit, reach the next authored checkpoint and verify save/load there works without repeating rewards.
- [ ] In a copied Story with declared string/session and collection keys, check Unicode/AltGr input, UTF-8 backspace, Ctrl+A, valid JSON types, the 16 KiB limit, and rejection of nested/oversized collections. Session-only changes retain save eligibility when rules do not change persistent state.
- [ ] Switch to command mode with Tab. Reject malformed JSON, `save`, an unknown command, and `award` with a string argument without mutation or save invalidation. Execute `{"do":"command.call","command":"award","args":[10]}`: exactly ten points and `awarded=true`, unchanged displayed line, and save refused until a new checkpoint. Hold Enter (no repeat), release and press again (another ten points); Tab+Enter changes mode without execution. Cycle through trace to return to state mode. Close/reopen and verify the command buffer remains.
- [ ] Check pointer capture, focus loss/resume, keyboard navigation, resize and supported DPI settings. Inspect long keys/values and multiple pages for clipping/readability. Record OS, GPU, layout and observed issues before signing off.
- [ ] In developer mode, Tab twice opens trace: Commands Demo `award` shows completed actions, subscribed state notifications and matched/skipped events in order. Enter/typing/Story clicks and F5/F9 do not execute or progress. Page navigation and Delete affect only history; the command buffer survives the three-mode cycle. Generate enough command activity to verify 128-row retention, dropped counts and readable clipping at supported DPI/window sizes. A successful load starts empty history, a failed load keeps the live history, and saving before/after trace-only operations has unchanged bytes. A normal launch retains no trace.
- [ ] Launch Commands Demo directly at `Next` with `--debug-tools --start-node Next` (or another exact authored node in a copied Story); verify `Next points 1` from defaults, without Start's two-point reward. Record the node/source/build. Confirm the title names the node and disables save/load. Try F5/F9 and authored save/load buttons at a checkpoint: normal slot/backup hashes stay unchanged. Relaunch normally to confirm ordinary save/load still works. Missing/case-mismatched node names refuse before a window. Repeat with CLI preview and the exported player's adjacent/explicit Story and artifact forms; inspect that node's presentation/audio and F12 trace. Node starts do not replace the other checklist's normal-session save tests.
