# Weftora Examples

**Status: PROPOSED authoring samples only (2026-10-08). No engine/CLI/player yet.** JSON schemas provisional; files cannot currently be run/exported. First target: Windows x64.

## Hello Story

[Open Hello Story](./hello-story/README.md) — small VN with branching Yarn, Story variable, screen, manifest, content catalogs and expected results.

```text
examples/hello-story/
├── manifest.json
├── dialogue/intro.yarn
├── dialogue/catalog.json
├── data/{catalog.json,game.json}
├── screens/{catalog.json,main_menu.json}
├── events/catalog.json
├── assets/{catalog.json,README.md}
└── tests/expected.json
```

## Future user workflow (commands not implemented)

From repo root, Windows PowerShell:

```powershell
Copy-Item -Recurse examples\hello-story examples\my-first-game
# Change Story id/version in manifest.json; edit Yarn and UI
weftora check examples\my-first-game
weftora run examples\my-first-game
weftora export examples\my-first-game
```

Files: manifest identifies Story/entrypoint/catalogs; Yarn stores lines/choices; screens define UI; catalog maps IDs to files; expected JSON holds future test outcomes. Authors edit Story, **not Engine Rust**.

Roadmap: Phase 0 validates format; Phase 2 runs branches; Phase 3 adds assets/audio/save; Phase 5 adds separate life-sim Story B.

Docs: [Architecture](../docs/architecture.md), [Roadmap](../ROADMAP.md), [Success Criteria](../docs/success-criteria.md), [Doc index](../docs/README.md).
