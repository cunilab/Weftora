# Hello Story — First Weftora Game

**Status: DESIGN FIXTURE. Not playable/exportable today.** No Weftora CLI/player/schema implementation exists. Catalog and screen JSON fields **provisional**; Yarn language syntax based on [Yarn Spinner 2.5](https://docs.yarnspinner.dev/2.5/getting-started/writing-in-yarn/lines-nodes-and-options). Rust adapter compatibility not verified.

## Expected game

**Courtyard Choice**: click Start → node `intro` → Mira offers two choices.

- **Talk with Mira** → `$metMira = true` → `MeetMira` → `Ending`: Mira says “See you tomorrow!”
- **Explore courtyard** → `$metMira = false` → `ExploreCourtyard` → `Ending`: narrator says “You leave before sunset.”

No image/audio/save yet. Later Stage 3 adds VN media and persistence.

## Read/edit

1. [manifest.json](./manifest.json) — Story ID, `intro` entrypoint, feature list, catalog paths; `^0.1.0` Engine API **hypothetical**, not released.
2. [dialogue/intro.yarn](./dialogue/intro.yarn) — nodes, options, `<<jump>>`, `<<set>>`; `Setup` contains `<<declare>>` without running.
3. [screens/main_menu.json](./screens/main_menu.json) — UI button invokes proposed `dialogue.start` at `intro`.
4. [data/game.json](./data/game.json) — game title, initial UI screen.
5. [dialogue/catalog.json](./dialogue/catalog.json) — proposed `entries` mapping content ID→file; other catalogs similar.
6. [tests/expected.json](./tests/expected.json) — both choice results; **planned test fixture only**, no test runner.

## Copy and customize (once CLI is implemented)

```powershell
Copy-Item -Recurse examples\hello-story examples\my-first-game
# Change manifest.json id to my.first_game
# Edit dialogue/intro.yarn and screens/main_menu.json
weftora check examples\my-first-game
weftora run examples\my-first-game
weftora export examples\my-first-game
```

Commands unavailable today. No Cargo, Rust game project or Bevy app required for future Story authors.

## Future acceptance

CLI must validate IDs/paths/entry node, reject invalid manifest/command/Yarn, execute both routes, bridge Yarn variable to canonical state, and keep compiled player SHA-256 unchanged after Story content edits. Later validate art/audio/save on Windows.

**No success criteria marked PASS** from design files alone. See [success criteria](../../docs/success-criteria.md), [assets plan](./assets/README.md), [all examples](../README.md).
