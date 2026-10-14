---
id: WFT-EVID-003
title: Phase 1 Evidence
status: proposed
---

# Phase 1 Evidence

Implementation and development-host results for the [Phase 1 checks](../success-criteria.md#phase-1--headless-kernel). This is **not Windows gate sign-off**. The required Windows CI job must run the fixtures, including junction and symlink escape, before Phase 1 can pass. Phase 0's [P0-10 deferral](./phase-0.md#p0-10-deferral-and-next-phase) remains tracked separately.

Development host: MacBookAir, macOS 27.0.1 (26A434), Apple M1; Rust 1.98.0 (pinned), `semver` 1.0.28, `yarnspinner` 0.9.0. Branch `feat/phase-1-headless-kernel`, based on `main` at `305196b` (merged PR #6). Exact implementation revisions are recorded by this page's commits and the linked PR/CI run when available.

| Check | Development-host result | Fixture/evidence |
| --- | --- | --- |
| P1-01 lifecycle | PASS | [Kernel tests](../../crates/weftora-core/tests/kernel.rs): exact ordered startup/reverse teardown trace, injected init and cleanup failures, no ready/play after failure, drop cleanup |
| P1-02 state | PASS | Kernel tests plus [state unit tests](../../crates/weftora-core/src/state.rs): all scalar type pairs, defaults, scopes, removal/reset, ordered notifications, finite float checks and overflow; 2,048 integer add/inverse cases |
| P1-03 commands | PASS | Kernel tests: argument count/type checks, unknown/duplicate command, invalid result, typed handler error, async yield/completion, cancellation before/after polling and resource drop; completed result preserved |
| P1-04 signals | PASS | Kernel tests: golden delivery order `a1,a2,b1`, queued nested `a,b`, unsubscribe, bounded loop error and no leftover queue |
| P1-05 catalogs | PASS | [Story fixtures](../../crates/weftora-story/tests/phase_one.rs): duplicate/missing IDs with file and JSON pointer, duplicate object keys with line; kernel lookup diagnostics preserve prior entry |
| P1-06 compatibility | PASS | [ADR fixture matrix](../../crates/weftora-story/src/compatibility.rs): patch/additive API, outside range, missing capability, unnamed/named prerelease, invalid comparator syntax; Story tests cover unknown schema, Story version and initial-state type mismatch |
| P1-07 Windows path policy | NOT_RUN on Windows | [Portable path/limit fixtures](../../crates/weftora-story/src/paths.rs) pass on macOS. Story tests add Windows-only junction escape, symlink escape and case-alias rejection; these have not run on this host |
| P1-08 shared loader | PASS | [CLI tests](../../apps/weftora-cli/tests/cli.rs): `check` and headless `run` reject an invalid package with identical errors, no trace activation; missing/invalid/unused choices fail |
| P1-09 deterministic replay | PASS | CLI tests compare [yes](../../apps/weftora-cli/tests/fixtures/headless-yes.jsonl) and [no](../../apps/weftora-cli/tests/fixtures/headless-no.jsonl) golden traces byte-for-byte in 10 fresh processes per branch |

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
