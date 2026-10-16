# Station Desk — custom screens demo

P4-06 uses existing `ui.screens/1` widgets: three authored base screens, a shared dialogue/choice stream, a green/gold theme and a modal pause overlay. No Rust extension, new widget or theme schema is needed. Theme values live in each screen's `color`, `background`, `size` and `layout` fields.

```sh
weftora check examples/screens-demo
weftora preview examples/screens-demo
weftora run examples/screens-demo --headless --choices 0
```

The player starts on **Menu** at the opening line. **Open activity desk** changes the base to a two-column desk without advancing dialogue. **Prepare for shift** updates `prepared`; its bound label becomes `true`. **Dialogue view** changes the layout again. **Continue** offers two Yarn choices; the station choice reads the prepared value, and the activity desk's task label then reads `Station shift`. Choosing rest instead sets `Rest day`. Menu and every base view share the same current dialogue and offered choices. Headless play does not execute UI actions, so its station branch starts without a prepared kit.

**Pause** blocks underlying buttons, digits and background progression. Tab/Shift+Tab stay within its buttons. **Resume** returns focus to the initiating Pause button when keyboard navigation is active. **Return to menu** replaces the base, clears the overlay and starts focus on Menu's first button. Screen navigation does not restart Yarn. Esc exits; the demo has no save actions.

## Same-player edit proof

Build the CLI/player once, then run this from the repository root. The output must be a new directory, preferably outside Cargo's cached `target` tree.

```sh
python3 scripts/verify-screen-edits.py --cli target/debug/weftora --player target/debug/weftora-player --out /tmp/weftora-screens-preview
```

Windows uses `python`, `.exe` executable paths and a fresh output path. CI uploads `weftora-windows-screens-preview-<sha>`. Its `baseline/` contains the player and authoring CLI; run `weftora-player.exe story` there. To view the edit from that directory, run `weftora-player.exe ../edit-ui/story`.

The proof changes only `ui/menu.jsonc`: purple background, 48-pixel padding, `STATION DESK / NIGHT` heading, and the first button renamed **Open compact log** and retargeted to Dialogue. It exports Hello Story, baseline and edited packages using the same player hash, validates the edit with the retained baseline player and asserts identical compiled dialogue/headless output. It does not prove native appearance or pointer behavior.

## Native checks queued in issue #5

- [ ] Open the baseline. Check green/gold Menu, two-column Activity and the full-width Dialogue panel. Switch among them without losing the displayed line or choices.
- [ ] Prepare, check `true`, choose the station shift and see the prepared-kit line. Return to Activity and check `Station shift`. Relaunch the player and choose rest; check `Rest day`.
- [ ] Tab/Shift+Tab through the desk, open Pause with Enter/Space, and cycle both directions within the overlay. Digits/background clicks/underlying desk clicks must not progress or prepare. Resume restores focus to Pause.
- [ ] From Pause, Return to menu clears the overlay; Tab enters only Menu buttons. Reopen the desk normally.
- [ ] Resize to 1024×768 and 1280×720. Check readable labels, choices and accessible buttons. Record screenshots and any clipping; automated model checks do not establish native layout.
- [ ] Launch the edited package using the baseline player. Check purple Menu, new heading/padding and Open compact log leading to Dialogue. Compare the player's SHA-256 with `same-player-proof.json`.

Automated renderer tests exercise authored button activation, canonical bindings, choice selection, themed Bevy components, overlay gating, focus restoration, Tab navigation and stale-origin rejection. CLI invalid fixtures reject an unknown widget, hidden-overlay missing binding, missing navigation target, malformed theme color and wrong state-action type before run/export output. Native rendering/picking/focus remains unverified until these Windows checks are recorded; it does not block development.
