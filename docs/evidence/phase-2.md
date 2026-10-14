---
id: WFT-EVID-004
title: Phase 2 Implementation Evidence
status: proposed
---

# Phase 2 Implementation Evidence

Phase 2 is **in progress**, not a passed gate. The first slice implements P2-03's compiled-dialogue boundary. Windows presentation, audio, input and performance checks remain required.

## Compiled-dialogue slice

- Source compilation moved into `weftora-yarn-compiler`; the player uses runtime-only `weftora-yarn`. `cargo xtask deps` checks normal/build dependency paths from the player and has a negative transitive-compiler fixture. It remains valid in an all-feature workspace graph.
- [ADR 0002](../adr/0002-versioning-compatibility.md#compiled-dialogue-format-1) defines artifact format 1, exact compiler identity, source hash, raw-body checksum and resource limits. Loader tests reject unknown formats before parsing current fields, corruption, unknown compiler identity, oversized input and malformed structural bytecode with recomputed checksums.
- Compiler tests recompile a fixture ten times and compare artifact bytes plus both branch event/state traces before and after serialization. These are in-process compilations; the earlier fresh-process CLI replay tests remain separate.
- Player loader tests consume a packaged artifact, accept an explicit preview artifact, and reject future formats, stale source edits, missing entrypoints and an actual over-limit file before calling Bevy. These tests prove activation validation; they do not open a window or test audio.
- CLI tests verify exported artifact identity/hash and source correspondence; a Unix fake-player fixture confirms the preview exists during the child process, its exit status propagates, and the temporary artifact is removed afterward. This preview process fixture is not claimed as Windows UI evidence.
- CLI and player share state/default, entrypoint, command schema, asset reference and command provenance checks. The CLI also validates its serialize/load round trip; export checks the copied package before reporting success.

## Verified revision

Code/test commit `9d1db87545c0fc0982bd0ffc6f2a563f27f57d50` passed all three jobs in [CI run 38537227113](https://github.com/cunilab/Weftora/actions/runs/38537227113): required Windows workspace tests/Clippy/format/docs/dependencies, required dependency audit, and macOS headless tests/dependency guard. Windows ran 143 tests with zero failures. Full raw [workspace test step](./artifacts/phase-2-windows/workspace-tests.txt) and [player dependency tree](./artifacts/phase-2-windows/player-dependencies.txt) are preserved, including timestamps and blank-line whitespace. The tree has no compiler or ANTLR dependency. The job reported Rust 1.98.0; Yarn core/compiler/runtime are pinned 0.9.0 and Bevy remains locked 0.20.0.

Claude Opus 5.5 at medium effort advised and reviewed the implementation using only Read/Glob/Grep. Its final focused verdict was **ship**. Codex implemented the changes and ran verification. Reports are local-only on MacBookAir under `/tmp/weftora-phase2-{advice,review,final-review}/`; the durable behavior and remaining limits are recorded here.

Latest local headless/compiler/CLI/xtask tests, workspace Clippy, dependency direction and documentation checks passed. An earlier overlapping native test/build used stale build inputs while review fixes were edited; that result is not used as evidence. The immutable CI checkout above is the full workspace test evidence. No new native player/renderer/audio behavior is claimed by this slice.

## Reproduction

```text
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask deps
cargo xtask docs
cargo tree -p weftora-player --edges normal,build --locked
cargo run -p weftora-cli -- check examples/hello-story
cargo run -p weftora-cli -- export examples/hello-story --out <empty-export-dir>
<export-dir>/weftora-player[.exe] <export-dir>/story --validate
```

A fresh direct source-folder launch now needs the CLI to compile a preview; the player has no compiler fallback. Existing exported folders from Phase 0 need re-exporting. CLI/compiler/player release identity enforcement is still a Phase 6 requirement, not proven by the existing fake-player export fixture.

## Outstanding checks and limits

| Check | State |
| --- | --- |
| P2-01 canonical state read by a real Engine presentation primitive | Automated ECS integration passes locally for choice → state → UI label → later Yarn condition; 174 Windows tests passed at the UI revision below |
| P2-02 file/node diagnostics and no-panic errors | OPEN; existing compiler/runtime panic containment is not the complete criterion |
| P2-03 versioned artifact and compiler-free player | PASS for automated criterion at `9d1db87`; see immutable CI, player negative-format fixture and dependency tree above |
| P2-04 views and transitions | Media slice implemented with explicit-time/ECS tests; native Windows scripted demo pending |
| P2-05 audio channels/fades/volume | Media slice implemented with ownership/gain/ECS tests; audible Windows device evidence pending |
| P2-06 declarative UI and focus | Implementation and automated ECS/negative-screen tests pass locally; maintainer native Windows checks pending |
| P2-07 pinned adapters and licence inventory | Exact Bevy/Yarn pins and [declared licence inventory](./artifacts/phase-2-adapter-licenses.tsv); all three jobs passed at `ffd0211c99aa76a46ea715e35abee5fab42a56e8` in [CI run 38550566712](https://github.com/cunilab/Weftora/actions/runs/38550566712), including 150 Windows tests |
| P2-08 Windows performance baseline | NOT_RUN |
| P2-09 image/audio/font catalogs | IMPLEMENTED on the asset-catalog branch; resolution/path/type/duplicate tests passed in the asset CI above; native image/audio/font evidence remains maintainer-owned |
| P2-10 UI renderer ADR | Decision recorded in [ADR 0005](../adr/0005-declarative-ui-renderer.md); accepted; native verification remains open |

Structural bytecode validation is not a complete stack/type verifier. Runtime stack/options and string-memory growth need bounded failure handling ([issue #8](https://github.com/cunilab/Weftora/issues/8)). Existing instruction budgets remain active; OOM is not caught by panic containment. No phase requirement is waived. P0-10 remains deferred under [issue #5](https://github.com/cunilab/Weftora/issues/5).

## Asset-catalog slice

Typed image/audio/font catalog entries are strict `{id,path}` records. A single asset catalog rejects cross-kind and legacy collisions with both source locations; generic data catalogs retain their own namespace. Shared CLI and renderer lookup checks declared kinds; legacy assets remain untyped and use runtime decoding. The sample keeps its original asset IDs, Yarn sources and golden traces while moving declarations into catalog files. Font fixtures prove ID/path resolution using placeholder bytes only, not font decoding or rendering. Bounded binary reads also reject media that grow past the per-file limit after inventory.

Windows manual tests are maintainer-owned and intentionally blank in [the Windows record](./phase-2-windows.md), tracked by [issue #5](https://github.com/cunilab/Weftora/issues/5). Agents do not request Windows access. The CI preview package is provided for those tests; its `--validate` step is automated activation validation, not proof of rendering, audio or input behavior.

The [licence inventory](./artifacts/phase-2-adapter-licenses.tsv) records all 619 packages in `cargo metadata --locked --all-features --offline`, including target-specific dependencies, their locked versions and declared SPDX licence metadata. It does not audit every licence file or replace the Phase 6 licence/advisory gate. No package lacked declared licence metadata in this snapshot.


## Declarative UI slice

The [UI v1 contract](../ui.md) adds strict versioned screen files, all eight widgets, bounded structure/layout validation, typed image/font references, canonical-state and dialogue bindings, and button actions. Shared CLI/player validation includes Yarn default types. Screen sources are export references. The sample adds a dialogue screen and modal overlay without changing its Yarn or headless golden traces. Stories without screens retain the original panel.

The adapter builds screen trees when the layer stack changes, patches labels in place, rebuilds choices on new offers and restores focus by stable widget ID. A bounded input queue routes modal actions and prevents multiple dialogue progression intents in one frame; background clicks use picking rather than raw mouse presses. Referenced image/font handles are cached after bounded Story reads and decoder validation. Fonts must register successfully with Bevy's own font collection; image 0.25.10 (already locked) checks dimensions and a conservative byte estimate before decoding. UI preloading is capped at 64 MiB; the dependency inventory's package set is unchanged.

On MacBookAir, the local full workspace suite passed 183 tests across 22 suites with zero failures. Workspace Clippy with warnings denied, formatting, dependency direction and documentation checks passed. Adapter tests prove choice → canonical state → bound label → later Yarn condition, UI action type/precision checks, modal routing, Tab from empty focus, no Space selection before keyboard navigation, focus restoration, in-place labels and retained notifications. These ECS fixtures do not exercise real window keyboard dispatch. Claude Opus 5.5 medium used only Read/Grep in advice/reviews; its final verdict was ship after input fixes. Codex performed every edit and check. Code/test revision `6e2f63b1357e9ac01267d5ffa3da7b716eaf2711` passed all three jobs in [CI run 38564553715](https://github.com/cunilab/Weftora/actions/runs/38564553715): 174 Windows tests, zero failures, 22 suites; workspace Clippy, format, docs and dependency checks; dependency audit; macOS checks. The [raw Windows test step](./artifacts/phase-2-windows/ui-workspace-tests.txt) preserves runner output. CI built/exported Hello Story, validated its compiled artifact and uploaded the debug preview. These are automated results; native rendering, keyboard dispatch, audio and performance remain unverified. The earlier asset-branch 150-test result applies only to its immutable revision. Native Windows fields remain blank. This implementation does not complete views/transitions, multichannel audio, VM memory-growth containment, or Windows performance measurement.


## Presentation-media slice

The [presentation v1 contract](../presentation.md) shares one Core parser/table among static Yarn checks, registered headless handlers and the presenter. Backend-free explicit-time models cover five view slots and four audio channels. Bevy draws centered position/scale/opacity transitions below UI, bounds resident/per-frame asset data, handles late audio sinks and applies canonical Settings gains. Legacy background/sound and Hello Story golden traces remain unchanged; new commands require `presentation.media/1`.

An isolated [scripted demo](../../examples/presentation-demo/README.md) uses generated CC0 rectangles/quiet tones, channel volume buttons and every required transition. On MacBookAir, the full workspace passed 200 tests with zero failures across 22 suites. Workspace Clippy with warnings denied, formatting, docs and dependency direction passed. Claude Opus 5.5 medium advised/reviewed with only Read/Grep; final focused verdict ship after default/live volume consistency fixes and streamed looping audio. Rodio's built-in LoopedDecoder replaces whole-track PCM buffering through a small Bevy asset adapter; tests verify typed source/Once playback and two exact WAV loop periods. Music/ambience loop boundary behavior on a real device, Vorbis loops and performance remain native checks. Windows media CI is pending. No actual audio, native window/input or performance result is filled in. The UI pipeline cache regression was fixed at `6728ec7` by exporting preview bundles outside Cargo's restored target directory.
