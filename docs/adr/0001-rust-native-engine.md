---
id: WFT-ADR-001
title: Proposed Rust-Native Engine Decision
type: architecture_decision
doc_version: 0.1.0
status: proposed
implementation: not_applicable
created: 2026-10-08
updated: 2026-10-08
last_reviewed: null
owner: Weftora Maintainers
scope: documentation
product_release: unreleased
related_pr: 1
supersedes: null
---

# ADR 0001 — Proposed Rust-native engine

**Status: proposed; no technical implementation or benchmarks.** PR #1 pivots prior Unity 6 LTS/Yarn foundation into standalone Rust-native Windows player + CLI + external Story packages. Require explicit maintainer decision.

## Drivers

One compiled player runs distinct VN and life-sim Stories. Creators edit dialogue, rules, screens and assets via prebuilt tools, without Rust/Cargo/Unity projects or per-game native binaries. Windows x64 first, later ports gated.

## Alternatives and tradeoffs

| Option | Strength | Cost/tradeoff |
| --- | --- | --- |
| Unity 6 LTS + Yarn | Mature asset/UI ecosystem and existing project direction | External Unity project/runtime dependency, engine release/licensing lifecycle |
| Godot + Story host | Open source 2D/UI/tooling | Custom authoring/package contract atop existing engine |
| Ren'Py based player | Mature VN authoring, export | Generic life-sim UI/rules require nontrivial extension |
| Rust core + Bevy backend + Yarn adapter (**proposed**) | Own runtime/API/distribution, independent headless core | High engineering cost; Yarn Rust, player/UI/export and creator UX unproven |

These are product-design tradeoffs, **not** evidence of Rust performance or delivery speed.

## Required proof/reversal gates

- P0: time-boxed real Yarn compiler/runtime choice probe *before* committing kernel dialogue/state APIs; document tested versions/license and fallback.
- P2: real Windows rendering/input/audio, compiled Yarn branching, canonical state bridge and source-located diagnostics. If unsupported, test replacement adapter or revisit Godot/Unity.
- P3: Story author runs/changes packaged VN through prebuilt player, same binary SHA; safe save/load; optional developer alpha only.
- P6: publish creator workflow costs, measurable startup/RAM/input/validation budgets, maintenance effort and clean-machine install evidence.
- **Reverse/revisit** if Yarn author contract cannot be met, ordinary Story rules require game-specific Rust compile, or measured Windows creator/maintainer burden exceeds simpler host-engine option. New stack decision requires new ADR and amended PRD/roadmap.

Rust memory safety does not prove story correctness. No promise of Rhai sandbox or source compatibility until tests.

## Change History

| Date | Version | Change | Reference |
| --- | --- | --- | --- |
| 2026-10-08 | 0.1.0 | Record proposed Rust pivot, alternatives and reversal tests. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
