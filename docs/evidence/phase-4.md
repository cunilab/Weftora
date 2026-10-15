---
id: WFT-EVID-009
title: Phase 4 Rules Evidence
status: proposed
---

# Phase 4 Rules Evidence

[PR #19](https://github.com/cunilab/Weftora/pull/19) implements the scalar condition/event foundation and JSONC Story integration described in [ADR 0008](../adr/0008-declarative-events.md). These are development-host and automated results, not native Windows acceptance. Phase 4 remains incomplete; generic Story commands, UI/host-write bridging, collections and Rhai are not implemented by this slice.

## Automated results

On MacBookAir, the full workspace run passed 296 reported tests with zero failures. The subsequently added checkpoint-loop regression also passed in a focused run. Workspace Clippy with warnings denied, formatting, dependency direction, documentation and `cargo deny check` passed. The CLI and player built with the locked dependency graph.

| Area | Verified result |
| --- | --- |
| P4-01 conditions | Six operator/type/boundary tests pass, including skipped invalid branches and exact integer comparison |
| P4-02 events/actions | Nine event tests pass, including priority/declaration order, FIFO parity, golden traces, cycles, typed errors and budget boundaries |
| Story files | Seven loader/bridge tests pass: JSONC diagnostics, capability/schema, duplicates, path policy, scope/size/depth limits, old-queue isolation and explicit signal/reset chains |
| Real headless execution | The rules demo produces ten byte-identical traces; interpolation precedes rule settling, then later dialogue reads the derived canonical value |
| Checkpoint replay | Prefix rules award once, resave produces identical slot/backup bytes, and writes before checkpoint capture are already included in the saved snapshot |
| Failed candidate | Runtime overflow publishes no CLI candidate output and preserves slot/backup bytes; the renderer model also preserves live state and save files |
| Resource limits | Rule feedback cycles stop; stale queue cursors fail; rule writes share the VM write/string counters; repeated checkpoint commands do not reset that cumulative write budget |

Reproduction:

```sh
cargo test --workspace --locked
cargo test -p weftora-cli --test rules checkpoint_loops_do_not_reset --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo xtask deps
cargo xtask docs
cargo deny check
cargo build -p weftora-cli -p weftora-player --locked
```

Claude Opus 5.5 medium provided tools-disabled advice and independent review. Review found a notification-history accounting issue; per-boundary accounting, stale-cursor rejection, shared VM charging and regressions address it. Follow-up found no remaining hard blocker and requested the checkpoint-loop regression, which passed. Codex performed all edits and execution.

## Content-only rule edit

The [macOS proof JSON](./phase-4-rules-edit-proof-macos.json) records three exports after one build: Hello Story, the rules demo, and a rules variant awarding twenty points with `saveSchema: 2`. No Rust build occurred between exports or edits. All three exported players share SHA-256 `d3b85335f03a4b2e04b6b36bcc51ff646c60408e992cfca1c603b7ba20ab51fe`.

The retained baseline player validated both rule packages using their corresponding compiled dialogue artifacts. Headless execution of the packages produced `After rules 10` and `After rules 20`, respectively. The compiled dialogue hashes stayed identical: only rule/manifest content changed. This proves automated package validation and headless mechanics on macOS, not a native visual run of either variant.

## Windows handoff

Windows CI runs the workspace and exports/validates `rules-demo`, compares its player hash with Hello Story, and uploads `weftora-windows-rules-preview-<sha>` with a matching CLI. Native baseline/edit/save-relaunch/error scenarios are queued in [issue #5](https://github.com/cunilab/Weftora/issues/5). No Windows run for this integration or maintainer observation is claimed here. Pending native acceptance does not block subsequent implementation.
