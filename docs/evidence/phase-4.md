---
id: WFT-EVID-009
title: Phase 4 Rules Evidence
status: proposed
---

# Phase 4 Rules Evidence

[PR #19](https://github.com/cunilab/Weftora/pull/19) implements the scalar condition/event foundation and JSONC Story integration described in [ADR 0008](../adr/0008-declarative-events.md). These are development-host and automated results, not native Windows acceptance. Phase 4 remains incomplete. Typed commands, UI state-write bridging, flat collection state and the bounded Rhai host below are implemented. Arbitrary external host-write bridging, deferred script features and native acceptance remain open.

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

## Typed commands and screen inputs

The next PR #19 development slice adds `rules.commands/1`: named scalar-typed parameters, argument forwarding, bounded static nested-call expansion, and calls from Yarn, events and screen buttons. UI `state.set` now settles rules as well. Save-enabled command targets remain Persistent-only. Persistent UI writes invalidate the current checkpoint until the next checkpoint; replay records no UI input and the save format is unchanged. Yarn interpolated numeric/bool arguments work; string arguments require literal text because the current VM tokenizes after interpolation.

On MacBookAir, the updated locked workspace run passed **312 reported tests**, zero failures (including bounded-child test executions). Workspace Clippy with warnings denied, formatting, dependency direction, documentation and `cargo deny check` passed; CLI/player builds passed. The later strengthened quoted-string value assertion passed in a focused CLI test. Flow now has ten event/sequence tests, Story rules have fifteen loader/bridge/command tests, and CLI rules have ten integration tests.

New coverage includes forward references; event/command ID overlap; strict scalar arguments and interpolation; nested source locations; partial writes on overflow; recursion/depth/expansion limits; oversized repeated leaf rejection; command/notification budget sharing; VM-write → command → notification ordering; immediate later Yarn reads; isolated candidate failure preserving output/slot bytes; gated UI inputs; 5000 separate button calls without a cumulative VM-budget failure; and save refusal after UI changes followed by successful next-checkpoint reconstruction.

Claude Opus 5.5 medium independently reviewed the source with tools disabled. Its early-copy identifier amplification and cumulative UI-budget findings were fixed and covered by regressions. Its interpolation concern led to the explicit Yarn string restriction. Follow-up reported no hard blockers; the conditional save-failure concern is covered by `fail_with_context` clearing capture and `save_point` rejecting unhealthy runners, and the loader checks UI action types/references before activation.

The [command content-only proof](./phase-4-commands-edit-proof-macos.json) records Hello Story, the command demo, and a four-point edited command with `saveSchema: 2`, exported after one build. All players share SHA-256 `b740b97ca8cb82026e8e27ac08b693dc8e0570837b93081dbd455d780c73fba1`. The retained baseline player validated the edited package; headless lines changed from `First points 2` / `Next points 3` to `First points 4` / `Next points 8`. Dialogue source and compiled-artifact hashes stayed identical. No build occurred between exports or edits; this is package/headless evidence, not a visual run.

Windows CI additionally exports/validates `commands-demo`, compares it with the same Hello Story player and uploads `weftora-windows-commands-preview-<sha>` with a matching authoring CLI. Concrete command/UI/save/edit/error checks are queued in issue #5. No native acceptance or Phase 4 gate is claimed.

## Flat collection state

The next PR #19 slice adds canonical flat scalar lists/maps under `state.collections/1`, with whole-value set/reset/remove through existing rule actions and commands. Maps serialize in sorted order. Decoding and Rust state writes enforce 256 entries, 16 KiB payload per collection, scalar-only children and bounded map keys. Explicit rule literal wrappers preserve maps containing `argument` keys. Yarn variables, command parameters, conditions, signals and text bindings remain scalar-only; the demo maintains authored scalar views. [ADR 0009](../adr/0009-flat-collection-state.md) records the contract and deferred entry operations/Rhai.

On MacBookAir, the latest locked workspace run passed **327 reported tests**, zero failures, including bounded-child executions. Workspace Clippy with warnings denied, formatting, dependency direction, documentation and `cargo deny check` passed; locked CLI/player builds passed. New tests cover scalar encoding compatibility, deterministic collection serialization, flat/size/duplicate/type rejection, atomic state/snapshot failures, signed-zero bits inside collections, rule literal disambiguation, static expansion payload charging, scalar boundary diagnostics, byte-identical checkpoint resaves and failed CLI/renderer candidates preserving live state/output/files.

Checkpoint capture and display arming now check the complete Persistent declaration set before cloning snapshots, including removed keys. An oversized payload/entry set reports a save-data refusal while dialogue continues. Regressions cover a payload above 8 MiB and 4097 declarations removed between capture and the first display. Claude Opus 5.5 medium reviewed source with tools disabled, found the removed-declaration gap, and confirmed its fix in a follow-up with no remaining blockers. Conditional concerns about scalar paths were resolved against the existing Flow validation and loader tests.

The [collection edit proof](./phase-4-collections-edit-proof-macos.json) records Hello Story, baseline collections and a three-item/schema-2 variant exported after one build. All player SHA-256 hashes are `ea1497dc55caa2ff6add97bb85d430096bdc4a57e473833db722e0733f0fc570`. The retained baseline player validated the edited package. Headless output changed from `Packed 2 items` to `Packed 3 items`; both next displays reset to zero, while inspected canonical state contains the expected list and schedule map. Compiled dialogue hashes stayed identical. Loading the old-schema save refused with no output or slot changes. No build occurred between exports or edits; this is package/headless evidence, not a native visual run.

Windows CI exports/validates `collections-demo`, compares its player hash with Hello Story and uploads `weftora-windows-collections-preview-<sha>` with the matching CLI. Concrete collection/UI/save/edit/error checks remain queued in issue #5. Native acceptance and the Phase 4 gate remain open and do not block development.

## Bounded Rhai commands

PR #19 adds the `weftora-script` adapter under `rules.rhai/1`, with explicit state/signal permissions, raw-engine helpers, shared dispatch/feedback operation and work allowances, depth/data limits, source diagnostics and fresh invocation scope. The same runner executes scripts from Yarn, event actions and screen commands, including isolated checkpoint candidates. [ADR 0010](../adr/0010-bounded-rhai-host.md) records the adapter-composition exception, resource contract and deferred features.

On MacBookAir, the locked workspace run passed **341 reported tests**, zero failures, including bounded-child executions. New tests cover compile and runtime source diagnostics, permissions/scope/type/data rejection, FIFO external command execution, shared budgets, button/event calls, deterministic resaves and CLI/renderer candidate isolation. Workspace Clippy with warnings denied, formatting, dependency direction, documentation and cargo deny passed; locked CLI/player builds passed. The advisory exception below remains explicit.

The [script edit proof](./phase-4-scripts-edit-proof-macos.json) records Hello Story, baseline scripts and a three-item/schema-2 variant exported after one build on MacBookAir. All player hashes are `1520a7dad743e083f510e102e50eed6d20f84f07d9509fcb110776baeeba160c`. The retained baseline player validates the edited package. Changing only the script's limit (plus the required save schema) changes headless output from `Packed 2 items` to `Packed 3 items`, with canonical schedule energy changing from 8 to 7. Both next displays reset to zero. Compiled dialogue hashes stay identical; loading an old-schema save refuses without output or slot changes. This is package/headless evidence, not a native visual run.

Tools-disabled Claude Opus 5.5 medium independently reviewed the implementation and found no confirmed blocker. Its AST statement-form concern was hardened explicitly; expression/statement/method reflection checks and anonymous-function rejection are covered. Recursion runs in a deadline-bounded child on a 1 MiB stack using the deepest accepted nested expression. The host uses conservative call/expression depth limits of 8/16. Validation already compiles scripts before activation. Per-dispatch versus per-display allowances are documented explicitly.

Rhai's mandatory smartstring dependency is subject to unmaintained advisory RUSTSEC-2026-0249, with no patched release. The specific `deny.toml` exception and ongoing maintenance risk are recorded in ADR 0010. No general advisory suppression was added.

Windows CI exports and validates `scripts-demo`, compares its player hash with Hello Story and uploads `weftora-windows-scripts-preview-<sha>` with the matching CLI. Issue #5 queues button/event/save/edit/error scenarios for the maintainer. Script-internal Engine calls, canonical seeded randomness, native acceptance and the Phase 4 gate remain open; Windows testing does not block development.
