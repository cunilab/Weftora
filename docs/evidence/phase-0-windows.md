---
id: WFT-EVID-002
title: Phase 0 Windows Evidence
status: proposed
---

# Phase 0 Windows Evidence

Windows x64 results for the [Phase 0 checks](../success-criteria.md#phase-0--spikes-and-walking-skeleton) that
the [Phase 0 evidence page](./phase-0.md) recorded as `NOT_RUN`. These runs close P0-01, P0-02, P0-03, P0-05,
P0-06 and P0-07 on Windows. P0-10 stays open; see [what is still outstanding](#what-is-still-outstanding).

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

## P0-03 CI blocks a merge

**P0-03 passes.** A red required job blocks the merge, and the administrator escape hatch is refused too.

The check needed an enforcement mechanism first. Before this run `main` had **no branch protection and no
rulesets** — the `(required)` text in `ci.yml` is a job label that GitHub does not read. PR #2 confirmed the gap:
it read `mergeStateStatus: UNSTABLE`, meaning "not all checks green, merge still allowed". Two repository rulesets
were created ([`p03-rulesets.txt`](./artifacts/phase-0-windows/p03-rulesets.txt)):

| ID | Name | Rules | Bypass |
| --- | --- | --- | --- |
| `25017317` | `main: must use PR, squash, CI` | `pull_request` (0 approvals, `allowed_merge_methods: ["squash"]`), `required_status_checks` (`Windows (required)`, `Dependencies (required)`), `non_fast_forward` | none |
| `25017474` | `main: review required (admins exempt)` | `pull_request` (1 approval, `require_last_push_approval`) | `OrganizationAdmin` |

They are split in two because GitHub bypass is **per ruleset, not per rule**: a single ruleset with an admin
bypass would have let admins skip the pull-request and status-check rules as well. Rule layering then does the
rest — a non-admin is subject to both rulesets, an admin only to `25017317`.

Architecture [§10](../architecture.md#10-drift-protection) names six merge-gate items; they run inside those two
jobs (`fmt`, `clippy`, `test`, `xtask deps`, `xtask docs` in *Windows*; `cargo deny check` in *Dependencies*). So
two probes — one per job — cover all six.

### Probe A — Windows red, Dependencies green

A deliberately mis-formatted `p003_fmt_probe` in `crates/weftora-api/src/lib.rs`; `cargo fmt --all -- --check`
is the Windows job's first step.

| Check | Result |
| --- | --- |
| Windows (required) | **FAILURE** |
| Dependencies (required) | SUCCESS |
| macOS headless (non-blocking) | SUCCESS |
| `mergeStateStatus` | **BLOCKED** |

```
$ gh pr merge 3 --squash
X Pull request cunilab/Weftora#3 is not mergeable: the base branch policy prohibits the merge.   (exit 1)

$ gh pr merge 3 --squash --admin
GraphQL: Repository rule violations found
Required status check "Windows (required)" is failing.                                            (exit 1)
```

### Probe B — Dependencies red, Windows green

`MIT`, `Apache-2.0` and `Apache-2.0 WITH LLVM-exception` removed from `deny.toml`'s licence allow list.

| Check | Result |
| --- | --- |
| Windows (required) | SUCCESS (all five steps green) |
| Dependencies (required) | **FAILURE** |
| macOS headless (non-blocking) | SUCCESS |
| `mergeStateStatus` | **BLOCKED** |

```
$ gh pr merge 4 --squash
X Pull request cunilab/Weftora#4 is not mergeable: the base branch policy prohibits the merge.   (exit 1)

$ gh pr merge 4 --squash --admin
GraphQL: Repository rule violations found
Required status check "Dependencies (required)" is failing.                                      (exit 1)
```

Full traces: [`p03-fmt-merge-blocked.txt`](./artifacts/phase-0-windows/p03-fmt-merge-blocked.txt),
[`p03-deny-merge-blocked.txt`](./artifacts/phase-0-windows/p03-deny-merge-blocked.txt).
Runs: [38441749677](https://github.com/cunilab/Weftora/actions/runs/38441749677) (A),
[38442678898](https://github.com/cunilab/Weftora/actions/runs/38442678898) (B).

Both probes were throwaway PRs from branches deleted afterwards; neither was merged.

**One caveat, recorded rather than hidden.** Two of the review rules' *semantics* are not provable from this
repository today. Both collaborators are `OrganizationAdmin`, so both bypass `25017474` and the 1-approval rule
currently binds nobody — it only bites once a non-admin collaborator is added. And `reviewDecision` kept reporting
`REVIEW_REQUIRED` on an admin's own PR even with bypass active; GitHub does not fold the viewer's bypass into the
PR's reported state (cli/cli#13388). Neither affects the merge gate itself, which `25017317` enforces with no
bypass actors — hence the `--admin` refusals above.

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

**The sandbox route is dead on this host.** Windows Sandbox was the intended Rust-free machine. It does not
boot: `BaseImages` has been empty since 2026-07-20, the feature already read `Enabled` before this work began
(so `Enable-WindowsOptionalFeature` was a no-op reporting `RestartNeeded: False`), and a reboot by the operator
cleared both `RebootPending` flags without provisioning the base image. Launches die with `0x80370106`, and
`<VGpu>Disable</VGpu>` — the most-upvoted fix in microsoft/Windows-Sandbox#101 — changed nothing. The breakage
predates this work by nearly three months. Details:
[`p10-sandbox-broken.txt`](./artifacts/phase-0-windows/p10-sandbox-broken.txt).

### Running under a Rust-free account instead

With the sandbox unavailable, the export was run under a freshly created local account, `weftora-p010`, which
has no `.cargo`, no `.rustup`, and does not resolve `cargo`, `rustc` or `rustup`. That is meaningful on this
host because **Rust is installed per-user only**: `C:\Users\andre\.cargo\bin` appears on *andre's user PATH*
and nowhere on the machine PATH, there is no HKLM uninstall entry, no `Program Files\Rust`, and the ACL on
`andre\.cargo` grants nothing to other users.

Under that account the exported player reported its identity (`--version`, exit `0`) and ran the Story with no
arguments — `Story::load` → yarn compile → Bevy init → Vulkan adapter → `Creating new window Hello Story` →
renderer running, alive 40s with no panic. The SHA-256 it printed is computed at runtime from its own bytes on
disk and matches the staged file. Logs:
[`p10-rustfree-account.txt`](./artifacts/phase-0-windows/p10-rustfree-account.txt).

**Still partial, for two stated reasons.** First, this is a fresh *account*, not a separate *machine*:
`C:\Users\andre\.cargo` is still on the same disk. The isolation is real at the account boundary but a reviewer
can object that the host is not literally Rust-free. Second, no frame was captured from that account — the
player lands in a separate logon session on a non-interactive window station, and `PrintWindow` refuses those
handles with `ERROR_INVALID_WINDOW_HANDLE` (1400) even when elevated. That is a Windows session boundary, not a
player fault, and the check's required evidence ("steps, machine description, log") is all present. Closing it
properly needs either an interactive logon as `weftora-p010` for a captured frame, or a genuinely separate VM.

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
| P0-10 export on a Rust-free machine | PARTIAL | Export is self-contained (7 files, no Cargo residue) and the full render pipeline runs under a Rust-free local account, but that is a fresh *account* on the same disk — not a machine with Rust uninstalled — and no frame was captured from it (session boundary blocks `PrintWindow`). Windows Sandbox, the intended route, has been unbootable on this host since 2026-07-20. |

P0-10 is the only check left open, and what remains is a *machine-boundary* question, not a question about the
export. Either an interactive logon as `weftora-p010` (one operator action, no reboot) or a separate VM closes
it; [Phase 0 is otherwise passing](./phase-0.md).
