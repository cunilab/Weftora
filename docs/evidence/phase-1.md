---
id: WFT-EVID-003
title: Phase 1 Evidence
status: proposed
---

# Phase 1 Evidence

Results for the [Phase 1 checks](../success-criteria.md#phase-1--headless-kernel). **Phase 1 passes the headless kernel gate** at implementation commit `727d36522dcf1939b2ae6ddbf536241ae3b0b3d1`, proposed in [PR #7](https://github.com/cunilab/Weftora/pull/7). All required checks passed on Windows x64 in [CI run 38521226907](https://github.com/cunilab/Weftora/actions/runs/38521226907); the [captured validation summary](./artifacts/phase-1-windows/validation.txt) includes the junction, symlink and ten-process replay results. No new rendering, audio or clean-machine export evidence is claimed. Phase 0's [P0-10 deferral](./phase-0.md#p0-10-deferral-and-next-phase) remains tracked separately.

Windows verification host: GitHub-hosted Windows Server 2025 Datacenter `10.0.26100`, x64; runner image `windows-2025-vs2026` version `20260925.250.1`; Rust 1.98.0 x86_64-pc-windows-msvc (pinned), `semver` 1.0.28, `yarnspinner` 0.9.0. Full workspace: 118 tests passed, zero failed. The required Dependencies job and non-blocking macOS headless job also passed.

Development host: MacBookAir, macOS 27.0.1 (26A434), Apple M1; Rust 1.98.0. Local headless tests, workspace Clippy, formatting, dependency boundaries, docs and `cargo deny check` passed. The additional local full workspace test build was stopped during renderer dependency compilation after the authoritative Windows run passed; it is not recorded as a local PASS.

| Check | Windows CI result | Fixture/evidence |
| --- | --- | --- |
| P1-01 lifecycle | PASS | [Kernel tests](../../crates/weftora-core/tests/kernel.rs): exact ordered startup/reverse teardown trace, injected init and cleanup failures, no ready/play after failure, drop cleanup |
| P1-02 state | PASS | Kernel tests plus [state unit tests](../../crates/weftora-core/src/state.rs): all scalar type pairs, defaults, scopes, removal/reset, ordered notifications, finite float checks and overflow; 2,048 integer add/inverse cases |
| P1-03 commands | PASS | Kernel tests: argument count/type checks, unknown/duplicate command, invalid result, typed handler error, async yield/completion, cancellation before/after polling and resource drop; completed result preserved |
| P1-04 signals | PASS | Kernel tests: golden delivery order `a1,a2,b1`, queued nested `a,b`, unsubscribe, bounded loop error and no leftover queue |
| P1-05 catalogs | PASS | [Story fixtures](../../crates/weftora-story/tests/phase_one.rs): duplicate/missing IDs with file and JSON pointer, duplicate object keys with line; kernel lookup diagnostics preserve prior entry |
| P1-06 compatibility | PASS | [ADR fixture matrix](../../crates/weftora-story/src/compatibility.rs): patch/additive API, outside range, missing capability, unnamed/named prerelease, invalid comparator syntax; Story tests cover unknown schema, Story version and initial-state type mismatch |
| P1-07 Windows path policy | PASS | [Portable path/limit fixtures](../../crates/weftora-story/src/paths.rs) and Windows-only junction escape, symlink escape and case-alias fixtures all passed; see the captured validation summary |
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
