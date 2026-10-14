---
id: WFT-ADR-006
title: ADR 0006 — Performance Baseline and PRD R15 Budgets
status: proposed
---

# ADR 0006 — Performance Baseline and PRD R15 Budgets

## Context

[PRD R15](../../PRD.md#5-requirements) sets provisional, unmeasured Windows budgets: cold start to menu
≤ 5 s, input-to-feedback p95 ≤ 100 ms, idle memory ≤ 750 MiB per sample Story, and `weftora check` on the
starter Story ≤ 2 s. [P2-08](../success-criteria.md#phase-2--dialogue-and-presentation) requires a Windows
performance baseline to be recorded and these budgets confirmed or revised by ADR.

A baseline was measured on a real Windows machine while gathering
[agent-run evidence](../evidence/phase-2-windows-agent.md). The measurements below come from a release build
of commit `c9f2aa0` on Windows 11 Pro 26300, Intel i7-13620H, NVIDIA RTX 4050 (driver 610.62, Vulkan), using a
PowerShell harness that launches the exported player and samples the process. Full logs are in
[`perf-release.txt`](../evidence/artifacts/phase-2-windows-agent/perf-release.txt).

## Decision: confirm the measured budgets; leave input p95 open

Record the baseline and **confirm** the three budgets that were measured on this machine, because each has a
clear margin:

| R15 budget | Measured (release, this machine) | Confirmed? |
| --- | --- | --- |
| Cold start ≤ 5 s | window-ready median 0.51 s, max 0.59 s warm; 1.86 s on the first-ever cold launch | **Yes** |
| Idle memory ≤ 750 MiB per sample Story | 371 MiB working set; ~625 MiB private bytes | **Yes** |
| `weftora check` on starter Story ≤ 2 s | median 11 ms, max 17 ms | **Yes** |
| Input-to-feedback p95 ≤ 100 ms | **not measured** | **No — remains open** |

Do **not** revise any budget. The input-to-feedback p95 line is **left unconfirmed** because it was not
measured: the only instrument available here is `PrintWindow` frame capture, whose per-frame cost exceeds the
engine latency being measured, so a number read from it would reflect capture overhead rather than input
latency. Measuring p95 honestly needs a dedicated harness — engine-side input timestamps or a
desktop-duplication capture path — and is a separate task.

## Consequences

- Cold-start, idle-memory and `check` budgets are confirmed by measurement on one machine; the numbers are a
  baseline, not a multi-run statistics package, and a reference-hardware re-check is still reasonable before
  a release gate.
- The p95 budget stays provisional and unmet-by-evidence; P2-08 is not fully closed until it is measured with
  a proper harness or the budget is explicitly revised here.
- If later measurements on other hardware or a real reference machine exceed these budgets, revisit this ADR
  to revise R15 rather than silently passing over it.
- This ADR records performance budgets only. It does not restate any renderer, format or compatibility
  contract; those live in their own ADRs.
