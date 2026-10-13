---
id: WFT-EVID-001
title: Phase 0 Evidence
status: proposed
---

# Phase 0 Evidence

Results for the [Phase 0 checks](../success-criteria.md#phase-0--spikes-and-walking-skeleton). Phase 0 is **not passed** until every check passes on Windows x64; macOS results below come from the maintainer's development host and do not count toward the gate on their own ([platforms](../platforms.md)).

**Environment (dev host):** macOS 27.0, Apple M1, 8 GiB, Metal; Rust 1.98.0 (pinned); Bevy 0.20.0; `yarnspinner` 0.9.0 (Bevy feature off). Branch `feat/phase-0-walking-skeleton`, 2026-10-13.

| Check | Result | Evidence |
| --- | --- | --- |
| P0-01 workspace builds, player reports identity | PASS (macOS); Windows CI NOT_RUN | `cargo check --workspace --locked`; `weftora-player --version` → `weftora-player 0.0.1 (sha256 abcc5d2e…)` |
| P0-02 headless crates free of Bevy/Yarn/Rhai | PASS (macOS); Windows CI NOT_RUN | `cargo xtask deps`; injected `bevy_platform` (normal, optional and Windows-only) into `weftora-core` → failed with the dependency path |
| P0-03 CI blocks on the merge gate | NOT_RUN | `.github/workflows/ci.yml` committed; needs a first GitHub run and an injected failure |
| P0-04 Yarn spike | PASS (headless, macOS) | `crates/weftora-yarn/tests/hello_story.rs`: both branches traced, `curious` true/false, Rust→Yarn state, syntax error with file+line, missing node, Int beyond ±2^24 rejected. Gaps below |
| P0-05 Bevy spike | PASS (macOS); Windows NOT_RUN | Window with background, Guide line, two choices, chime; screenshots taken during the run |
| P0-06 walking skeleton | PASS (macOS); Windows NOT_RUN | `weftora run examples/hello-story`; key `1` → "You looked closer, so the state key "curious" is now true." |
| P0-07 edit Yarn, same player | PASS (macOS); Windows NOT_RUN | Edited a line, `weftora check` OK; player SHA-256 unchanged (`abcc5d2e…`) |
| P0-08 loader rejects paths outside the Story | PASS | `weftora-story` tests: absolute, `..`, backslash, drive, UNC, `:`, symlink escape |
| P0-09 authoring format chosen | PASS | [ADR 0004](../adr/0004-story-authoring-format.md): JSONC |
| P0-10 export runs without Rust | PARTIAL | `weftora export` → `dist/hello-story/` with `weftora-export.json`; exported player starts with no arguments on macOS. Clean Windows machine NOT_RUN |
| P0-11 go/no-go recorded | PASS | [ADR 0001](../adr/0001-rust-native-engine.md#phase-0-go-no-go-decision) |

## Yarn spike: gaps found

- The Yarn compiler panics on some malformed input (for example `<<set $x = >>`). `weftora-yarn` catches the panic and reports a file-level error without a line number. Report upstream; improve the message in Phase 2.
- `<<jump>>` with no target compiles without an error.
- Yarn's own visit-tracking variables (`$Yarn.Internal.Visiting.*`) live outside canonical state. They must be saved with the game in Phase 3.
- `OpCode` is not re-exported, so the static command scan matches the run-command opcode by number; a test fails if it changes.
- The Yarn `VariableStorage` trait exposes `bevy_platform::HashMap`, so `weftora-yarn` depends on `bevy_platform` (a small utility crate, not the engine). Allowed for an adapter.
- Fallbacks remain those in [architecture §6](../architecture.md#6-dialogue-yarn-spinner); none needed so far.

## Codex audit, 2026-10-13

An adversarial review by Codex (review mode) reported 8 defects. Each was confirmed by a reproduction before fixing; fixes were written by Codex workers, reviewed by Claude Opus, and have regression tests.

| Finding | Reproduction | Fix |
| --- | --- | --- |
| `export --out` inside the Story recursed | 69 nested copies, 210 MB | Refuse overlapping or symlinked `--out`; remove only what export created on failure |
| Hidden folders silently dropped from export | `.dialogue/` missing from the export | References may not contain hidden, `.` or `..` segments; export verifies every reference was copied |
| Manifest symlink escaped the Story | `check` exited 0 | All references, including the manifest, reject symlink/junction segments and case mismatches |
| State/Yarn type mismatch panicked | panic in the Yarn VM | State checked against declared Yarn types before start and in `weftora check`; runtime panics fail the runner |
| `i64::MIN` panicked in the precision guard | overflow panic | Range check; keys that do not round-trip through the Yarn name mapping are rejected |
| `<<jump Start>>` hung | no return | Vendored `yarnspinner_runtime` with an instruction budget; adapter caps consecutive commands (1,000) and cumulative instructions between lines/choices |
| Unreadable sound crashed the player | `bevy_audio` unwraps its decoder | Sounds validated with the same decoder first; Story error shown |
| Reference-style links bypassed the docs gate | broken `[g]: missing.md` passed | Docs gate parses CommonMark references |

During the audit `cargo deny` also flagged a newly published vulnerability, RUSTSEC-2026-0344 (`ruzstd` decompression bomb), reached only through Bevy's 3D features. Bevy now builds with `2d`, `ui` and `audio` only; `hello-story` re-verified on macOS.

## To finish Phase 0 on Windows

1. Push the branch and let CI run (P0-01, P0-02, P0-03); inject one failure to prove it blocks.
2. On a Windows x64 machine with the release build: `weftora run examples/hello-story`, play both branches, edit a line and confirm the player hash is unchanged (P0-05 to P0-07). Record OS build, GPU, driver and audio device.
3. `weftora export examples/hello-story --out dist\hello-story`, copy the folder to a Windows machine without Rust, run `weftora-player.exe` (P0-10).
