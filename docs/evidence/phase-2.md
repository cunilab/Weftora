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
| P2-01 canonical state read by a real Engine presentation primitive | NOT_RUN; earlier adapter tests cover state but do not prove the new UI/view integration |
| P2-02 file/node diagnostics and no-panic errors | OPEN; existing compiler/runtime panic containment is not the complete criterion |
| P2-03 versioned artifact and compiler-free player | IMPLEMENTED; current commit/CI results to be attached after verification |
| P2-04 views and transitions | NOT_RUN |
| P2-05 audio channels/fades/volume | NOT_RUN |
| P2-06 declarative UI and focus | NOT_RUN |
| P2-07 pinned adapters and licence inventory | PARTIAL; Yarn subcrates are exact-pinned, full Phase 2 inventory not yet recorded |
| P2-08 Windows performance baseline | NOT_RUN |
| P2-09 image/audio/font catalogs | NOT_RUN |
| P2-10 UI renderer ADR | NOT_RUN |

Structural bytecode validation is not a complete stack/type verifier. Runtime stack/options and string-memory growth need bounded failure handling ([issue #8](https://github.com/cunilab/Weftora/issues/8)). Existing instruction budgets remain active; OOM is not caught by panic containment. No phase requirement is waived. P0-10 remains deferred under [issue #5](https://github.com/cunilab/Weftora/issues/5).
