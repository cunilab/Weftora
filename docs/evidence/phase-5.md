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
| P5-03 | Isolated macOS build and 362 default Engine test passes; ten expected fixture failures; required Windows CI passed | No cold-build or native rendering claim |
| P5-04 | 34 CLI and 16 renderer-model matrix rows pass for both real Stories; failed loads preserve live state/media/navigation and save-directory bytes | Optional native observation in issue #5; P5-05 compatibility rows remain separate |
| P5-05 | A→B and B→A save refusal; pay edit requires schema 2 | Remaining ADR 0002 version rows against both Stories |

## Scenarios and checkpoints

The work/buy/eat route ends at Home/hour 14/energy 3/coins 2, with permit+receipt. Work/no-shopping/rest ends at 13/2/4; visiting without work ends at 11/4/0; staying home ends at 9/6/0. Seed 0 pays 4 once. Boundary fixtures check arrivals before 9, at 11 and exactly at closing hour 12, and energy exactly 2 versus 1 at Market. Late, underpowered or permit-less visits cannot work or advance the pay stream. The Market checkpoint asserts both schedule-map and scalar open flags. All prose remains narration rather than being accidentally parsed as Yarn speaker labels.

At `day.home`, `day.market`, `day.shift`, `day.return` and `day.end`, CLI tests load/resave byte-identically and finish with uninterrupted final state. Player model tests spend resources after each saved checkpoint before comparing restored state/display and slot/backup bytes, repeat load at Shift, and check Journal save refusal/close-to-save/F9 reset. Alternate loaded branches can skip shopping or work without introducing pay or RNG draws. Travel costs, shift pay, inventory edits and RNG are reconstructed exactly once. Cross-Story CLI loads fail specifically with `save belongs to another Story`, publish no candidate dialogue and preserve the original slot; the export proof additionally preserves existing backups.

## Verification on MacBookAir

The locked complete suite (`cargo test-all`) with serial test execution passed **372 reported tests**, zero failures, including the deadline-bounded child fixtures. Six focused Story B CLI tests and the strengthened player model test also passed. Workspace Clippy with warnings denied, formatting, dependency direction, documentation and cargo deny passed; locked CLI/player builds passed. The existing specific Rhai/smartstring advisory exception remains; this slice adds no dependencies.

Tools-disabled Claude Opus 5.5 medium independently reviewed the Story content, tests, proof script and Windows handoff. It found no broken documented route, and identified weak replay evidence. The final tests mutate gameplay state before load, repeat Shift loads, assert the schedule map opens, check exact time/energy boundaries, allow alternate loaded branches and test schema refusal in cargo. The proof identifies CLI trace provenance separately from native-player validation. Its manifest edit preserves JSONC formatting/comments with a guarded scalar replacement. Author-required schema bumps are documented as an obligation the Engine cannot infer from changed mechanics.

## Same-player and edit proof

The [automated report](./phase-5-story-b-proof-macos.json) records Story A, Story B and edited-pay exports on one prebuilt player, SHA-256 `6f25d427a0c57476333672abf744cf1a0dfce5baaac117177d860ac1efbf054b`. The retained Story A player validates every package. Gameplay completion traces are produced by the shared headless CLI runner; they are not native player walkthroughs. No Rust build occurs between exports/edits.

Story A finishes its repair route. Story B's baseline work/buy/eat route reaches 14/3/2 with one receipt and one seeded draw. Changing only Rhai base pay from 3 to 5, plus the required save schema 1→2, changes earned pay 4→6 and end coins 2→4. Compiled dialogue, RNG, hour, energy and inventory stay unchanged. A→B, B→A and old-schema loads refuse for their expected identity/migration reasons without candidate output or slot/backup hash changes. The script ran with Python optimization enabled; proof checks use unconditional failures.

Windows CI runs `scripts/verify-life-sim.py` after the existing Story A proof and uploads `weftora-windows-life-sim-preview-<sha>` with one retained player/CLI, both Stories, edited content, traces and report. Exact native completion, route, checkpoint, Journal, resize, edit and negative-load steps remain unchecked in issue #5. No native appearance, input, performance, full compatibility matrix or Phase 5 gate is claimed.

## Engine independence and test-suite boundary

The default Engine suite and authored-Story scenarios are now explicit. The CLI's `story_a`, `life_sim` and `story_matrix` test targets require `story-tests`; the renderer's authored replay and matrix tests are feature-gated too. No production Rust path is conditional on this feature. `cargo test-all` enables both packages' test features and remains the required complete suite in CI. Clippy runs in both configurations. Existing Story tests are retained, with no fixture-presence early returns or copied Story replicas.

`scripts/verify-engine-independence.py` makes an isolated copy from Git's tracked and nonignored working-file inventory, rejecting symlinks/aliases, preserving working changes, and excluding only the two exact Story directories. It verifies copy hashes, required lock/vendor content and original source hashes after the run. The source inventory identifies the tested working snapshot; it does not pretend an uncommitted run is a clean commit build. The original Story folders are never renamed or deleted.

In that copy it runs the default locked workspace build and test commands. The full Story-test listing must include all fourteen expected names before stripping. Explicitly enabling the suites in the stripped copy must fail all ten CLI Story tests and four renderer Story tests specifically because the Story roots are absent. These are intentional negative controls; the normal Engine test command must succeed. Workspace-package artifacts are cleaned before the build because a shared Cargo target can retain test binaries with original manifest paths; dependency cache may be reused. Shared workspace artifacts are released on success or failure so later commands in the original checkout cannot borrow those copied-path test binaries. The top-level cached player is also removed before building, and the recreated player is copied out immediately after the build so later control builds cannot replace the tested binary. Explicit A/B paths and no-argument launch next to an absent `story/` each run twice, requiring exit 1, no stdout, deterministic manifest/root diagnostics and no panic/backend initialization.

The proof output must be outside the checkout. `--reuse-target` openly reuses Cargo cache; the alternate target directory does not imply a cold registry/source-cache run. Generic test fixtures and small capability demos remain in the copy: P5-03 removes Story A/B, not every test asset. Windows CI uploads only report/inventory/logs and the tested standalone player, not the temporary workspace or target tree. This adds no manual Windows blocker and does not claim native rendering acceptance.

The [MacBookAir report](./phase-5-engine-independence-macos.json) records clean source commit `352c4891277fc278a7213d1befdcf1848a35766d`: stripped build succeeded, **362 reported Engine tests passed**, all ten explicitly enabled authored tests failed for their own missing-root diagnostics, and the three missing-Story launch cases each returned deterministic exit 1 twice. Both workspace cache resets succeeded and original source hashes stayed unchanged. The restored original checkout then passed **372 reported tests** with `cargo test-all`, Clippy in both configurations, formatting, dependency direction, documentation and cargo deny; six Python proof-helper tests also passed.

Required [Windows CI run 39104884378](https://github.com/cunilab/Weftora/actions/runs/39104884378) passed on the same source head, including the isolated proof ([Windows report](./phase-5-engine-independence-windows.json), **349 reported Engine tests**). Its `baseCommit` identifies the GitHub pull-request merge checkout. Automated preactivation errors do not establish native gameplay, rendering or input acceptance.

## A/B failure matrix (P5-04)

The shared `tests/support/story_matrix.rs` helper copies each real Story into a temporary directory and adds a test-only Persistent `matrix_probe` integer. It never edits the examples. The CLI matrix and renderer matrix remain behind `story-tests`; the Engine independence runner now requires all fourteen authored tests and their missing-root negative controls.

| Layer | Cases per Story | Assertion |
| --- | --- | --- |
| CLI package preflight | Malformed manifest; unsupported manifest schema; incompatible API range; missing capability; missing dialogue file; traversal to an existing external Yarn file; missing entrypoint, initial screen or state binding; unknown command; Rhai syntax error | Exact failure reason, exit 1, no stdout for check/run/export; failed export creates no package and preserves an existing output sentinel |
| CLI load | Corrupt save; unsupported save format; real save from the other Story; save schema bump; removed checkpoint; Rhai runtime exception after a candidate state write | Exit 1, no candidate output, all save-directory names and bytes unchanged, including the backup and persistent writer lock |
| Player-model load | The six load cases above, plus oversized restored bound text and a new invalid image decoded during media restore | F9 with an overlay open preserves state identity/content, dialogue/revision/save eligibility, overlay, settled view samples, audio cue IDs/tokens/volumes and all save files; repairing the input then permits a real load |

The script cases inject a bounded command into each Story's first checkpoint, including Story A which does not normally use Rhai. The runtime variant passes static validation, writes `matrix_probe = 99` and throws; live state is deliberately changed to 77 before the player load. The display row uses a valid, checksummed save containing 4097 bytes for a bound string. The media row requires `prepare_load` to succeed before F9 encounters the new undecodable image, exercising failure after candidate preparation. Story A must have nonempty live audio so media preservation is not an empty comparison. The successful retry must replace the live state object, restore probe 0 and reset navigation.

These tests cover preactivation checks and failed checkpoint reconstruction. Installing revised Story metadata in the test harness does not implement or claim runtime hot switching between Stories. Headless CLI execution does not validate rendered UI; the renderer-model rows cover that boundary. Native window, audio output, input devices and Windows hardware acceptance remain in issue #5. P5-05's remaining compatible revision/ADR rows remain separate.

Focused commands (add `-- --nocapture` for individual `PASS Story/layer/case` rows):

```sh
cargo test --locked -p weftora-cli --features story-tests --test story_matrix
cargo test --locked -p weftora-render-bevy --features story-tests --lib story_matrix
```

Required Windows CI runs the complete suite and captures these focused row logs in `weftora-windows-story-matrix-<sha>`. The focused MacBookAir run passed all 34 CLI rows and 16 player-model rows; full-suite and platform evidence is recorded separately below.

The [MacBookAir report](./phase-5-story-matrix-macos.json) records source commit `541b1bd05d87eb0eadef5a5e28366cc2e1f6a89d`: the complete locked suite passed **374 reported tests**, both Clippy configurations and locked workspace build passed, and formatting (including included fixture files), dependency direction, documentation and cargo deny passed. All twelve authored tests are present in the full inventory and absent from the default Engine inventory. Six proof-helper tests passed. [Focused row excerpts](./phase-5-story-matrix-macos.log) record the 34 CLI and 16 player rows. Tools-disabled Claude Opus 5.5 medium reviewed the fixture and restore assertions; later display/media failures, existing-file traversal, complete save-directory comparisons and successful retries were hardened. [Windows CI](https://github.com/cunilab/Weftora/actions/runs/39194118874) runs the same matrix and updated twelve-test isolation controls; platform results are reported by that run rather than inferred from the Mac run.

The Windows full-suite and focused matrix steps also passed on this source head: [50 Windows row excerpts](./phase-5-story-matrix-windows.log) were verified against the downloaded CI artifact (34 CLI, 16 player-model). The overall run, including the updated source-isolation proof, is tracked separately by GitHub; these automated results do not imply native hardware acceptance.

## Compatible revisions and version fixtures (P5-05)

Two additional feature-gated matrix tests cover the ADR 0002 rows due through Phase 5 for both Stories. Each copy uses a stable test image ID at `scene.repair` / `day.shift`. Story version 0.1.0→0.1.1 changes the first checkpoint line, replaces the PNG bytes and adds a Persistent integer with default 23, keeping the save schema and checkpoint/node map unchanged. Old slot and backup bytes are retained. CLI continuation must finish with the baseline route/state plus the new default. A fresh player-model session loads that old noninitial checkpoint, replaces deliberately changed live state, displays the patched text, allocates a newly decoded image with different pixels and reconstructs the attempt counter / pay and RNG exactly once. This tests loading a revision in a new session, not runtime hot reload.

| ADR 0002 row | Both-Story evidence / scope |
| --- | --- |
| Engine patch, API unchanged | Valid old save fixture records writer release 0.0.0 with API 0.1.0; current CLI accepts it. This models diagnostic metadata, not an alternate historical executable. |
| Compatible API within range | Compatibility evaluator accepts 0.1.0 and 0.1.1 against each Story's requirements/capabilities. API inputs are modeled; no additive Engine binary is built. |
| API outside range / missing capability; unknown manifest schema | Existing P5-04 check/run/export refusal rows for both Stories; additionally the evaluator rejects 0.2.0 against each declared range. |
| Unnamed prerelease | Actual declared ranges reject 0.1.1-beta.1 with range/version/guidance; an explicitly opted-in range accepts 0.1.1-beta.2. |
| Unknown dialogue artifact format | Valid compiled artifact accepts; format 999 refuses specifically. Enhanced same-player proof also checks the retained standalone player's preactivation path for each revised package. |
| Compatible text/asset patch with stable save contract | CLI and fresh renderer session tests above; extended export proof completes old saves against real revised A/B packages using patched text and a new default. Real Story A also changes authored night-image bytes; Story B has no authored assets, so its image mutation is a test fixture. |
| Removed checkpoint / save-schema bump | Existing P5-04 CLI and player refusals preserve all save-directory bytes and live state. No migration is registered; the supported behavior is clear refusal. |
| Cross-Story, corrupt save and unknown save format | Existing bidirectional CLI and player matrix; real A/B saves refuse on identity. |
| Downgrade; mismatched export tool releases | ADR rows explicitly introduced in Phase 6, still open. Older writer diagnostics are not a downgrade fixture. |

The enhanced `scripts/verify-life-sim.py` retains one player across baseline A/B, changed-pay B and compatible A/B exports. It verifies changed compiled-dialogue bytes, patched checkpoint text, exact final canonical state with only the added field, untouched save-directory hashes and explicit future-artifact refusals. `--validate` checks package activation without a window; gameplay completion traces still come from the headless CLI. No rebuild occurs between these exports.

The focused compatibility run passed eight CLI policy/continuation rows and two renderer rows locally. Final proof/full-suite results are recorded after validation. Required Windows CI captures compatibility rows in the existing Story-matrix logs and includes compatible exports/saves/future-format fixtures in the life-sim preview artifact. Native acceptance remains in issue #5.
