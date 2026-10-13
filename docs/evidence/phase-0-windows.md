---
id: WFT-EVID-002
title: Phase 0 Windows Evidence
status: proposed
---

# Phase 0 Windows Evidence

Windows x64 results for the [Phase 0 checks](../success-criteria.md#phase-0--spikes-and-walking-skeleton) that
the [Phase 0 evidence page](./phase-0.md) recorded as `NOT_RUN`. These runs close P0-01, P0-02, P0-05, P0-06 and
P0-07 on Windows. P0-03 and P0-10 stay open; see [what is still outstanding](#what-is-still-outstanding).

All artefacts on this page are committed under
[`artifacts/phase-0-windows/`](./artifacts/phase-0-windows) so every claim below is a link to a
captured file, not a recollection.

## Verification host

| | |
| --- | --- |
| OS | Microsoft Windows 11 Pro, build 26200 (`10.0.26200`) |
| Machine | Axioo Pongo 750 |
| CPU | 13th Gen Intel Core i7-13620H, 10 cores / 16 logical |
| RAM | 15.7 GiB |
| GPU | NVIDIA GeForce RTX 4050 Laptop GPU, driver 32.0.16.1062 (NVIDIA 610.62) |
| GPU (secondary) | Intel UHD Graphics, driver 32.0.101.5768 |
| Audio | Speakers (Realtek(R) Audio); Realtek High Definition Audio, status OK |
| Renderer used by Bevy | Vulkan, on the NVIDIA discrete GPU |
| Toolchain | rustc/cargo 1.98.0 x86_64-pc-windows-msvc, pinned by `rust-toolchain.toml` |
| Commit | `cc0d6e402d34caf24b06e97075cbcbb4f9a28869` (`feat/phase-0-walking-skeleton`) |

Full capture: [`host-environment.txt`](./artifacts/phase-0-windows/host-environment.txt) and
[`toolchain.txt`](./artifacts/phase-0-windows/toolchain.txt).

## Merge gate on Windows

The same five steps CI runs on `windows-latest`, run locally on this host at the PR commit:

| Step | Result | Log |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS (3 s) | [`merge-gate-summary.txt`](./artifacts/phase-0-windows/merge-gate-summary.txt) |
| `cargo xtask deps` | PASS (47 s) | [`merge-gate-summary.txt`](./artifacts/phase-0-windows/merge-gate-summary.txt) |
| `cargo xtask docs` | PASS (2 s) | [`merge-gate-summary.txt`](./artifacts/phase-0-windows/merge-gate-summary.txt) |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS (143 s) | [`cargo-clippy.txt`](./artifacts/phase-0-windows/cargo-clippy.txt) |
| `cargo test --workspace --locked` | PASS (602 s), 98 passed / 0 failed across 15 suites | [`cargo-test.txt`](./artifacts/phase-0-windows/cargo-test.txt) |

`cargo deny` was **not** run on this host (not installed); the required Dependencies job covers it on
`ubuntu-latest` and is green on PR #2.

## P0-01 and P0-02

- **P0-01** — release build at the pinned toolchain with the committed lockfile.
  `weftora-player --version` reported `weftora-player 0.0.1 (sha256 8fee045540de95572f92c1b91e37d8e8d5b0eea8f3609739eeb8860456b1ab74)`,
  which is byte-identical to the SHA-256 of the file on disk. [`p01-identity.txt`](./artifacts/phase-0-windows/p01-identity.txt)
- **P0-02** — `cargo xtask deps` passed on Windows. Headless crates stay free of Bevy, Yarn and Rhai.

## P0-05 Bevy spike

A window renders the Story background, the speaker name, a dialogue line and two numbered choices, and the
Story's `<<sound sfx.chime>>` fires.

| Frame | What it shows |
| --- | --- |
| [`p06-branchA-choices.png`](./artifacts/phase-0-windows/p06-branchA-choices.png) | background, "Guide", and both choices `1. Yes, show me.` / `2. Not right now.` |

**Sound.** Bevy logs nothing for played audio, and this host has no WASAPI loopback device (no
`virtual-audio-capturer`; only `dshow`/`openal`), so a microphone capture was taken instead. It is
corroborating, not conclusive: player runs recorded a mean of `-22.6 dB` / `-22.0 dB` with `0.0 dBFS` peaks
against a `-41.7 dB` mean / `-16.7 dBFS` quiet-room control. The microphone's AGC clips, so this is not relied
on alone.

The deterministic proof is the negative test: with `assets/chime.wav` replaced by 64 bytes of garbage, the
player built the same `rodio` decoder `bevy_audio` uses and rejected it with a controlled Story error and no
panic, which proves the audio path is genuinely exercised on Windows:

```
ERROR weftora_render_bevy: asset `sfx.chime` is not readable audio: The format of the data has not been recognized.
```

Full log: [`p05-badsound-error.txt`](./artifacts/phase-0-windows/p05-badsound-error.txt). No
`panic`/`RUST_BACKTRACE` appears in the output.

## P0-06 walking skeleton, both branches

`weftora run examples\hello-story` launched the prebuilt player (SHA-256 `8fee0455…`). Input was delivered to
the window and each state captured, producing six distinct frames per branch:

| Step | Branch A (chose `1`) | Branch B (chose `2`) |
| --- | --- | --- |
| line 1 | `e80566ea…` | `e80566ea…` |
| line 2 | `abc0f102…` | `abc0f102…` |
| choices | `2ef13d6e…` | `2ef13d6e…` |
| **result** | **`79c72bdd…`** | **`fde914bb…`** |
| closing | `8551cb59…` | `8551cb59…` |
| end | `8adbb294…` | `8adbb294…` |

Every frame is byte-identical between the branches **except the result frame**, which is exactly what a state
branch should look like: the choice wrote `$curious`, and the later `<<if $curious>>` read it.

| Branch A result | Branch B result |
| --- | --- |
| [`p06-branchA-result-curious-true.png`](./artifacts/phase-0-windows/p06-branchA-result-curious-true.png) — "You looked closer, so the state key "curious" is now true." | [`p06-branchB-result-curious-false.png`](./artifacts/phase-0-windows/p06-branchB-result-curious-false.png) — "Fair enough. The state key "curious" stays false." |

Traces: [`p06-branchA-trace.txt`](./artifacts/phase-0-windows/p06-branchA-trace.txt),
[`p06-branchB-trace.txt`](./artifacts/phase-0-windows/p06-branchB-trace.txt), with per-frame hashes in
[`p06-branchA-frames.csv`](./artifacts/phase-0-windows/p06-branchA-frames.csv) and
[`p06-branchB-frames.csv`](./artifacts/phase-0-windows/p06-branchB-frames.csv).

## P0-07 edit Yarn, same player

`examples/hello-story/dialogue/intro.yarn` was edited to replace the result line with the marker
`P0-07-WINDOWS-EDIT-9d4c`:

- `weftora check` on the edited Story: exit `0`, `OK: dev.weftora.hello-story v0.1.0 - 2 nodes, 2 assets, 2 commands`.
- The edited text rendered on screen: [`p07-edited-line-same-player.png`](./artifacts/phase-0-windows/p07-edited-line-same-player.png).
- Player SHA-256 **before** the edit: `8fee045540de95572f92c1b91e37d8e8d5b0eea8f3609739eeb8860456b1ab74`
- Player SHA-256 **after** the edit and run: `8fee045540de95572f92c1b91e37d8e8d5b0eea8f3609739eeb8860456b1ab74` — identical.

The Yarn file was restored afterwards and `git status --porcelain` in the worktree was empty. Full trace:
[`p07-trace.txt`](./artifacts/phase-0-windows/p07-trace.txt).

## P0-10 export

`weftora export examples\hello-story --out …` exited `0` and wrote exactly 7 files — `weftora-player.exe`,
`weftora-export.json` and 5 story files — with no `.rs`, Cargo or `target/` residue.
[`p10-export-trace.txt`](./artifacts/phase-0-windows/p10-export-trace.txt) records a SHA-256 per file;
`weftora-export.json` carries `exportFormat`, `cli{version,sha256}`, `player{file,sha256}`,
`story{id,storyVersion}` and a hash for every story file.

The exported `weftora-player.exe` was then launched with **no arguments** (it must find `story/` beside itself)
and with `cargo`/`rustc`/`rustup` removed from `PATH` and `RUSTUP_HOME`/`CARGO_HOME`/`RUSTC`/`CARGO` unset.
It reported its identity and played the Story through the full frame sequence, reproducing the same rendered
frames as the repo-built player:
[`p10-exported-player-running.png`](./artifacts/phase-0-windows/p10-exported-player-running.png).

**Still partial.** The check's wording is "a Windows machine with no Rust or Cargo **installed**". Windows
Sandbox is installed on this host but not provisioned —
`C:\ProgramData\Microsoft\Windows\Containers\BaseImages` is absent and enabling the feature needs an elevated
shell, which this agent does not have — so a genuinely Rust-free machine was not available. Details:
[`p10-sandbox-blocked.txt`](./artifacts/phase-0-windows/p10-sandbox-blocked.txt).

## Method: driving the player without stealing focus

`weftora-player` exposes no `--headless`, `--trace` or `--no-window` flag and prints no dialogue or state
trace, so the only evidence of a state change is what the window renders. Two Windows specifics made naive
capture produce invalid evidence, and are recorded here because they will bite again:

1. `Get-Process … .MainWindowHandle` returns a **16x16 invisible stub window**, not the Bevy window. Targeting
   it sent keystrokes to whatever else had focus (the operator's own terminal). The real window must be found
   by enumerating top-level visible windows of the player's PID and taking the first one at least 200x200.
2. Because a background process cannot reliably call `SetForegroundWindow`, input is delivered with
   `PostMessage(WM_KEYDOWN/WM_KEYUP)` straight to that window handle — winit processes it without focus — and
   frames are captured with `PrintWindow(PW_RENDERFULLCONTENT)`, which reads the window regardless of what
   covers it. Neither step touches the operator's foreground window.

Diagnostic evidence for both is in the branch traces above, which record the hwnd and title found for every
run.

## What is still outstanding

| Check | State | Why |
| --- | --- | --- |
| P0-03 CI blocks a merge | PARTIAL | Needs a deliberately failing commit in a throwaway PR to prove the required job blocks the merge. Not run — no push was made to the repository. |
| P0-10 export on a Rust-free machine | PARTIAL | Export proven self-contained and runnable with Rust scrubbed from the environment; a machine with Rust truly *uninstalled* still needs a second VM or an elevated shell to provision Windows Sandbox. |

With P0-03 and P0-10 still partial, [Phase 0 remains not passed](./phase-0.md).
