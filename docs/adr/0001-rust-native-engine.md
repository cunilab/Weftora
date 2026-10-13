---
id: WFT-ADR-001
title: ADR 0001 — Rust-Native Engine
status: proposed
---

# ADR 0001 — Rust-Native Engine

## Context

The first design for this repository was a Unity 6 LTS framework with Yarn Spinner, where each game was a Unity project. PR #1 replaces it with a standalone engine: a prebuilt player and CLI that run external Story packages, so authors never need a game engine project or a compiler.

The drivers are:

- one compiled player runs a visual novel and a life-sim unchanged;
- authors edit dialogue, rules, screens and assets with prebuilt tools only;
- the runtime, file formats and distribution are owned by the project;
- Windows x64 ships first; other platforms come later.

## Options

| Option | Strengths | Costs |
| --- | --- | --- |
| Unity 6 LTS + Yarn Spinner (previous plan) | Mature editor, UI, asset pipeline; official Yarn Spinner support | Authors need Unity; engine licence and release lifecycle outside our control; "prebuilt player + external Story" fights Unity's build model |
| Godot 4 + Story host | Open source, strong 2D and UI, exports to many platforms | We still build the Story format, loader and tools, but on top of someone else's runtime; GDScript/C# extension model |
| Ren'Py | Decades of visual-novel authoring and export | Python-based; life-sim rules and custom UI are possible but fight the VN model; not our runtime |
| **Rust core + Bevy adapter + Yarn adapter (chosen, proposed)** | Own runtime, formats and distribution; headless testable core; single small binary | Highest engineering cost; Yarn Rust port is community-maintained and work-in-progress; Bevy has breaking releases every few months; UI, export and creator UX all unproven |

These are product tradeoffs, not evidence that Rust is faster to deliver.

## Decision (proposed)

Build the Rust-native engine, but treat it as a **bet with explicit exit points**, not a commitment.

To limit the main risks:

- **Bevy churn:** Bevy is confined to adapter crates; the headless core never sees it. Upgrade Bevy deliberately, at most once per phase, never mid-phase.
- **Yarn port maturity:** use only the Bevy-independent `yarnspinner` crates, not `bevy_yarnspinner`, so Bevy upgrades never wait on a third party. Fallbacks are listed in [architecture §6](../architecture.md#6-dialogue-yarn-spinner).
- **Scope:** the roadmap is a walking skeleton. Phase 0 must get a real Story on screen through the real CLI and player.

## Go/no-go gates

At the end of each gate phase the maintainer writes a short go/no-go decision in this ADR. A gate that is clearly stuck also triggers that review early. Silently continuing past a failed gate is not allowed.

| Gate | Must be true | If not |
| --- | --- | --- |
| End of Phase 0 | `weftora run examples/hello-story` shows text and a choice that changes state, on Windows, through `yarnspinner` + Bevy | Try the Yarn fallbacks; if the stack itself fails, revisit Godot |
| End of Phase 2 | Full Yarn integration, views, audio and declarative UI work on Windows; performance baseline is inside or near budget | Revisit the UI renderer or the stack |
| End of Phase 3 | An author changes Story A without touching the player binary; saves survive a Story patch | Stop and re-plan before building rules |
| End of Phase 6 | A non-Rust user creates and exports a game on a clean Windows machine | Do not release; fix or re-scope |

**Revisit the whole decision** if Story rules start needing game-specific Rust, if the Yarn author contract cannot be met, or if maintenance cost clearly exceeds a host-engine option. Changing stack needs a new ADR that supersedes this one.

## Phase 0 go-no-go decision

**2026-10-13 — provisional GO.** On the macOS development host, `weftora run examples/hello-story` drives the prebuilt player through the `yarnspinner` 0.9 + Bevy 0.20 stack: background, lines, a choice that changes canonical state, sound, and an exported folder that runs on its own. No Yarn fallback was needed; the gaps found are listed in the [Phase 0 evidence](../evidence/phase-0.md). The decision becomes final when the Windows checks in that page pass; a Windows failure in rendering, audio or export reopens this review.

## Consequences

- Rust memory safety does not prove story correctness; behaviour is proven by tests.
- No promise of a Rhai sandbox for untrusted mods.
- The previous Unity documents are superseded by this PR's PRD, roadmap and architecture.
