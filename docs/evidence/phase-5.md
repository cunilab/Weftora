---
id: WFT-EVID-010
title: Phase 5 Story B Evidence
status: proposed
---

# Phase 5 Story B Evidence

This records automated implementation coverage for [Phase 5](../success-criteria.md#phase-5--second-story-proves-reuse). Native player completion, rendering/input/resize and the Phase 5 gate remain open. Windows checks stay queued in [issue #5](https://github.com/cunilab/Weftora/issues/5) without blocking development.

## Market Day

[Story B](../../examples/life-sim/README.md) is a finite life-sim day across Home and Market. Declarative travel commands advance time and spend energy; a `state.changed.hour` event invokes a Rhai schedule rule. Work availability also checks energy and permit membership. Work pays from a canonical seeded integer stream; shopping/eating edit a flat inventory and coins/energy. The schedule is a flat map with a scalar `shift_open` view. A custom dashboard binds live values while Yarn queries inventory membership/length directly. Journal is a modal help overlay. The slice adds no production Rust, dependencies, public contract or save-format changes.

| Criterion | Automated coverage | Remaining |
| --- | --- | --- |
| P5-01 | Two locations, time/schedule, resources, inventory, conditional work and custom screen; four complete scenario routes | Native walkthrough |
| P5-02 | Same-player exports/activation validation for A/B, shared-runner completion, pay-edit proof | Completion of both Stories in the native player on the recorded binary |
| P5-03 | Runner implemented for isolated build/tests, missing-Story exits, mandatory test inventory and expected-failure controls; verification in progress | Completed platform report; no cold-build claim |
| P5-04 | Existing engine negative suites plus cross-Story/schema refusal below | Dedicated broader A/B package/ID/script/version matrix and live-state preservation evidence |
| P5-05 | A→B and B→A save refusal; pay edit requires schema 2 | Remaining ADR 0002 version rows against both Stories |

## Scenarios and checkpoints

The work/buy/eat route ends at Home/hour 14/energy 3/coins 2, with permit+receipt. Work/no-shopping/rest ends at 13/2/4; visiting without work ends at 11/4/0; staying home ends at 9/6/0. Seed 0 pays 4 once. Boundary fixtures check arrivals before 9, at 11 and exactly at closing hour 12, and energy exactly 2 versus 1 at Market. Late, underpowered or permit-less visits cannot work or advance the pay stream. The Market checkpoint asserts both schedule-map and scalar open flags. All prose remains narration rather than being accidentally parsed as Yarn speaker labels.

At `day.home`, `day.market`, `day.shift`, `day.return` and `day.end`, CLI tests load/resave byte-identically and finish with uninterrupted final state. Player model tests spend resources after each saved checkpoint before comparing restored state/display and slot/backup bytes, repeat load at Shift, and check Journal save refusal/close-to-save/F9 reset. Alternate loaded branches can skip shopping or work without introducing pay or RNG draws. Travel costs, shift pay, inventory edits and RNG are reconstructed exactly once. Cross-Story CLI loads fail specifically with `save belongs to another Story`, publish no candidate dialogue and preserve the original slot; the export proof additionally preserves existing backups.

## Verification on MacBookAir

The locked workspace run with serial test execution passed **372 reported tests**, zero failures, including the deadline-bounded child fixtures. Six focused Story B CLI tests and the strengthened player model test also passed. Workspace Clippy with warnings denied, formatting, dependency direction, documentation and cargo deny passed; locked CLI/player builds passed. The existing specific Rhai/smartstring advisory exception remains; this slice adds no dependencies.

Tools-disabled Claude Opus 5.5 medium independently reviewed the Story content, tests, proof script and Windows handoff. It found no broken documented route, and identified weak replay evidence. The final tests mutate gameplay state before load, repeat Shift loads, assert the schedule map opens, check exact time/energy boundaries, allow alternate loaded branches and test schema refusal in cargo. The proof identifies CLI trace provenance separately from native-player validation. Its manifest edit preserves JSONC formatting/comments with a guarded scalar replacement. Author-required schema bumps are documented as an obligation the Engine cannot infer from changed mechanics.

## Same-player and edit proof

The [automated report](./phase-5-story-b-proof-macos.json) records Story A, Story B and edited-pay exports on one prebuilt player, SHA-256 `6f25d427a0c57476333672abf744cf1a0dfce5baaac117177d860ac1efbf054b`. The retained Story A player validates every package. Gameplay completion traces are produced by the shared headless CLI runner; they are not native player walkthroughs. No Rust build occurs between exports/edits.

Story A finishes its repair route. Story B's baseline work/buy/eat route reaches 14/3/2 with one receipt and one seeded draw. Changing only Rhai base pay from 3 to 5, plus the required save schema 1→2, changes earned pay 4→6 and end coins 2→4. Compiled dialogue, RNG, hour, energy and inventory stay unchanged. A→B, B→A and old-schema loads refuse for their expected identity/migration reasons without candidate output or slot/backup hash changes. The script ran with Python optimization enabled; proof checks use unconditional failures.

Windows CI runs `scripts/verify-life-sim.py` after the existing Story A proof and uploads `weftora-windows-life-sim-preview-<sha>` with one retained player/CLI, both Stories, edited content, traces and report. Exact native completion, route, checkpoint, Journal, resize, edit and negative-load steps remain unchecked in issue #5. No native appearance, input, performance, full compatibility matrix or Phase 5 gate is claimed.

## Engine independence and test-suite boundary

The default Engine suite and authored-Story scenarios are now explicit. The CLI's `story_a` and `life_sim` test targets require `story-tests`; the renderer's two authored replay tests use `cfg(all(test, feature = "story-tests"))`. No production Rust path is conditional on this feature. `cargo test-all` enables both packages' test features and remains the required complete suite in CI. Clippy runs in both configurations. Existing Story tests are retained, with no fixture-presence early returns or copied Story replicas.

`scripts/verify-engine-independence.py` makes an isolated copy from Git's tracked and nonignored working-file inventory, rejecting symlinks/aliases, preserving working changes, and excluding only the two exact Story directories. It verifies copy hashes, required lock/vendor content and original source hashes after the run. The source inventory identifies the tested working snapshot; it does not pretend an uncommitted run is a clean commit build. The original Story folders are never renamed or deleted.

In that copy it runs the default locked workspace build and test commands. The full Story-test listing must include all ten expected names before stripping. Explicitly enabling the suites in the stripped copy must fail all eight CLI Story tests and the two renderer Story replay tests specifically because the Story roots are absent. These are intentional negative controls; the normal Engine test command must succeed. Workspace-package artifacts are cleaned before the build because a shared Cargo target can retain test binaries with original manifest paths; dependency cache may be reused. Shared workspace artifacts are released on success or failure so later commands in the original checkout cannot borrow those copied-path test binaries. The top-level cached player is also removed before building, and the recreated player is copied out immediately after the build so later control builds cannot replace the tested binary. Explicit A/B paths and no-argument launch next to an absent `story/` each run twice, requiring exit 1, no stdout, deterministic manifest/root diagnostics and no panic/backend initialization.

The proof output must be outside the checkout. `--reuse-target` openly reuses Cargo cache; the alternate target directory does not imply a cold registry/source-cache run. Generic test fixtures and small capability demos remain in the copy: P5-03 removes Story A/B, not every test asset. Windows CI uploads only report/inventory/logs and the tested standalone player, not the temporary workspace or target tree. This adds no manual Windows blocker and does not claim native rendering acceptance.
