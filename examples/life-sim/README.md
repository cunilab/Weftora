# Market Day — Story B

A short life-sim built entirely from Story files. Home and Market share a custom dashboard; time, energy, coins, a flat inventory, a schedule map and seeded pay use the existing rules/script/query/save adapters. There is no game-specific Engine code or new dependency.

```sh
weftora check examples/life-sim
weftora preview examples/life-sim
weftora run examples/life-sim --headless --choices 0,0,0,0
```

Use Continue/Space for lines, displayed choice numbers for activities, Tab/Shift+Tab and Enter/Space for buttons. Journal opens a modal help panel; close it to resume. Esc closes an overlay; otherwise press Esc twice within three seconds to quit. Choice numbers on screen are 1-based; headless option IDs are 0-based, including unavailable options.

## A day

Start at Home at hour 8 with 4 energy, no coins and a permit. Visiting Market costs one hour and one energy. A `state.changed.hour` event runs `refresh_schedule`: the morning shift opens at Market from 9 until 12, provided it has not been worked. Work additionally needs 2 energy and the permit. Manifest defaults are canonical; Yarn declarations provide variable types and fallback defaults. The schedule map and `shift_open` scalar view update through the same boundary; Yarn reads inventory length/membership with scalar query functions.

Work takes two hours, costs 2 energy, adds a receipt and pays base 3 plus a seeded integer bonus from 0 to 2. Seed 0 pays **4**. A snack costs 2 coins and one hour. Return travel costs one hour and one energy; eating a snack at Home consumes it, restores 3 energy and takes an hour. Rest takes an hour and restores 2 energy, capped at 6. This is one finite day, with no real-time clock, automatic new day or economic simulation.

| Headless option IDs | Route | End hour | Energy | Coins | Inventory |
| --- | --- | --- | --- | --- | --- |
| `0,0,0,0` | Market → work → buy snack → Home → eat | 14 | 3 | 2 | permit, receipt |
| `0,0,1,1` | Market → work → Home → rest | 13 | 2 | 4 | permit, receipt |
| `0,1,1` | Market → Home → rest | 11 | 4 | 0 | permit |
| `1` | Stay Home and rest | 9 | 6 | 0 | permit |

## Checkpoints

The first line at `day.home`, `day.market`, `day.shift`, `day.return` and `day.end` is saveable. Save with the dashboard button or F5; load with Load/F9. Progressing past that first display closes the safe point. Journal must be closed to save; loading while it is open closes it and restores the dashboard. UI navigation alone does not change game mechanics.

Market travel, shift work and return travel are reconstructed at their checkpoint openings. Replay tests verify that time/energy costs, coins, RNG and inventory happen exactly once and resave byte-identically. Authors must bump `saveSchema` for gameplay-mechanics edits, including pay/seed/time rules; the Engine cannot detect behavioral changes automatically. Compatible screen/text edits retain the schema. See the repository's checkpoint contract in `docs/ui.md` and ADR 0007.

## Same-player proof and Windows preview

Build CLI/player once, then from the repository root run:

```sh
python3 scripts/verify-life-sim.py --cli target/debug/weftora --player target/debug/weftora-player --out /tmp/weftora-life-preview
```

Use `python` and `.exe` paths on Windows. Output must be a new directory outside Cargo's cached target tree. CI uploads `weftora-windows-life-sim-preview-<sha>` containing Story A, Story B and an edited-pay variant, plus traces and `same-player-proof.json`. From `story-a/`, launch:

```powershell
./weftora-player.exe story
./weftora-player.exe ../story-b/story
./weftora-player.exe ../edit-pay/story
```

One player is retained in `story-a/`, beside the authoring CLI. The proof validates all packages with it and completes both Stories through the shared headless runner. The pay edit changes only `let base_pay = 3` to 5 plus saveSchema 2; pay becomes 6 and end coins become 4, with the same player, dialogue, RNG, hour, energy and inventory. Cross-Story and old-schema loads refuse without output or slot/backup changes. This does not prove native player completion; that remains in issue #5.

## Windows checks queued in issue #5

- [ ] Launch both Stories using the retained player. Complete Story A's repair route and Story B's work/buy/eat route; record the player hash and screenshots. Story B's final HUD is Home/hour 14/energy 3/coins 2, with no open shift.
- [ ] Relaunch Story B and try the other routes in the table. Check conditional choices, labels and schedule status; unavailable activities must not execute. On separate disposable copies, change manifest `state.hour.default` to 11, `state.energy.default` to 2, or `state.inventory.default` to `[]`; the Market work choice must be unavailable.
- [ ] Save/load at all five first checkpoint lines. Travel/work/purchase/eating results must not double; repeat F9 at the shift and check earned 4, coins 4, hour 11, energy 1. Continue to the baseline end state.
- [ ] Use Journal with keyboard and mouse while lines and choices are visible. Underlying progression/choice input must be blocked; close returns focus. Save while open refuses; F9 restores the saved dashboard.
- [ ] Resize to 1024×768 and 1280×720. Check readable HUD, dialogue and accessible choices/buttons with real mouse hit checks.
- [ ] Launch edited pay with the retained player. Work earns 6 and the eat route ends with 4 coins; load a baseline schema-1 save into that variant using a disposable save directory with the authoring CLI and confirm refusal. Leave real saves intact.

Native Windows appearance/input/performance, the broader negative/version matrix and the workspace-with-both-Stories-removed proof remain open. These acceptance checks do not block development.
