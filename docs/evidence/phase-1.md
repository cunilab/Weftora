---
id: WFT-EVID-003
title: Phase 1 Evidence
status: proposed
---

# Phase 1 Evidence

Results for the [Phase 1 checks](../success-criteria.md#phase-1--headless-kernel), proposed in [PR #7](https://github.com/cunilab/Weftora/pull/7). The original implementation `727d365` passed 118 Windows tests in [CI run 38521226907](https://github.com/cunilab/Weftora/actions/runs/38521226907). That run's [selected validation summary](./artifacts/phase-1-windows/validation.txt) is historical evidence, not a raw log or proof of the later integration fixes.

A read-only Claude review found that the original lifecycle trace owned no services and that several PASS descriptions blurred primitive tests and integration. The revised headless runtime now owns actual Story resources and observes real state changes through the signal bus. Regression tests cover partial-start rollback, forward-schema errors, complete state diagnostics and duplicate references; whole-loader tests cross the actual file-count and total-size boundaries. **Windows verification of this revision is pending.**

Development host: MacBookAir, macOS 27.0.1 (26A434), Apple M1; Rust 1.98.0 (pinned), `semver` 1.0.28, `yarnspinner` 0.9.0. Headless tests and workspace Clippy pass locally. No new rendering, audio, OneDrive, deduplication or clean-machine export run is claimed. Phase 0's [P0-10 deferral](./phase-0.md#p0-10-deferral-and-next-phase) remains tracked separately.

| Check | Current development-host result | Fixture/evidence |
| --- | --- | --- |
| P1-01 lifecycle | PASS | [Actual Story startup test](../../apps/weftora-cli/src/headless.rs): failure injected after real state/commands/observers/runner initialization, cleared resources and released state; CLI missing-node startup emits no gameplay. [Kernel tests](../../crates/weftora-core/tests/kernel.rs): exact ordered startup/reverse teardown trace, injected init and cleanup failures, no ready/play after failure, drop cleanup |
| P1-02 state | PASS | Kernel tests plus [state unit tests](../../crates/weftora-core/src/state.rs): all scalar type pairs, defaults, scopes, removal/reset, ordered notifications, finite float checks and overflow; 2,048 integer add/inverse cases |
| P1-03 commands | PASS | Kernel tests: argument count/type checks, unknown/duplicate command, invalid result, typed handler error, async yield/completion, cancellation before/after polling and resource drop; completed result preserved |
| P1-04 signals | PASS | Headless golden traces observe actual `state.changed` delivery; kernel tests: golden delivery order `a1,a2,b1`, queued nested `a,b`, unsubscribe, bounded loop error and no leftover queue |
| P1-05 catalogs | PASS | [Story fixtures](../../crates/weftora-story/tests/phase_one.rs): duplicate or absent record IDs at load, both duplicate declaration locations, duplicate object keys with line; missing-ID lookup API reports caller location and preserves prior entries. There is no general content-reference schema yet |
| P1-06 compatibility | PASS | [ADR fixture matrix](../../crates/weftora-story/src/compatibility.rs): patch/additive API, outside range, missing capability, unnamed/named prerelease, invalid comparator syntax; Story tests cover unknown schema even with future fields or missing legacy fields, Story version and initial-state type mismatch |
| P1-07 Windows path policy | PENDING on Windows | Whole-loader tests cross actual per-file/count/total-size limits. The entry limit uses a smaller synthetic budget. Unicode/ASCII collision policy is tested logically on all hosts; physical collision rejection runs on case-sensitive development volumes. Windows junction/symlink/case-alias tests passed in the original run and must pass again on this revision; no physical case-sensitive Windows collision fixture is claimed |
| P1-08 shared loader | PASS | [CLI tests](../../apps/weftora-cli/tests/cli.rs): `check` and headless `run` reject an invalid package with identical errors, no trace activation; missing/invalid/unused choices fail |
| P1-09 deterministic replay | PASS | CLI tests compare [yes](../../apps/weftora-cli/tests/fixtures/headless-yes.jsonl) and [no](../../apps/weftora-cli/tests/fixtures/headless-no.jsonl) golden traces byte-for-byte in 10 fresh processes per branch, plus ten runs of the [scoped Story](../../apps/weftora-cli/tests/fixtures/scoped-story/manifest.jsonc) exercising all three scopes, Yarn defaults, data-catalog loading and ordered signal notifications |

## Repeatable commands

```sh
cargo test -p weftora-api -p weftora-core -p weftora-story -p weftora-yarn -p weftora-cli --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask deps
cargo xtask docs
weftora run examples/hello-story --headless --choices 0
weftora run examples/hello-story --headless --choices 1
```

The headless run uses source Yarn and records presentation requests without rendering or audio playback. It does not establish new Windows player evidence. Loader package limits and runtime contracts are defined in [architecture](../architecture.md#4-story-package), with Windows filename rules checked against [Microsoft documentation](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file).
