---
id: WFT-DOCS-001
title: Weftora Documentation Index
status: proposed
---

# Weftora Documentation Index

Each document's header shows whether it is proposed or accepted. What actually works is recorded only as evidence against the [success criteria](./success-criteria.md).

| Document | ID | What it owns |
| --- | --- | --- |
| [Product Requirements](../PRD.md) | WFT-PRD-001 | Scope, users, requirements, non-goals |
| [Roadmap](../ROADMAP.md) | WFT-ROADMAP-001 | Phase order and tasks |
| [Architecture](./architecture.md) | WFT-ARCH-001 | Crates, boundaries, Story package, runtime semantics |
| [Success Criteria](./success-criteria.md) | WFT-ACCEPT-001 | Pass/fail checks per phase and evidence rules |
| [Platform Plan](./platforms.md) | WFT-PLAT-001 | Release target, development hosts, future ports |
| [Documentation Standards](./standards.md) | WFT-STD-001 | How docs are written and changed |
| [ADR 0001 — Rust-native engine](./adr/0001-rust-native-engine.md) | WFT-ADR-001 | Why Rust/Bevy/Yarn, go/no-go gates |
| [ADR 0002 — Versioning and compatibility](./adr/0002-versioning-compatibility.md) | WFT-ADR-002 | Version domains, save compatibility |
| [ADR 0003 — Story rules language](./adr/0003-story-rules-language.md) | WFT-ADR-003 | Declarative actions + Rhai, collection state |
| [ADR 0004 — Story authoring format](./adr/0004-story-authoring-format.md) | WFT-ADR-004 | JSONC for non-Yarn Story files |
| [ADR 0005 — Declarative UI renderer](./adr/0005-declarative-ui-renderer.md) | WFT-ADR-005 | Bevy UI choice and adapter boundary |
| [ADR 0006 — Performance baseline and R15 budgets](./adr/0006-performance-baseline.md) | WFT-ADR-006 | Windows perf baseline; confirms measured R15 budgets, leaves p95 open |
| [Presentation primitives v1](./presentation.md) | WFT-MEDIA-001 | View/audio commands, transitions, canonical volume and bounds |
| [Declarative UI v1](./ui.md) | WFT-UI-001 | Screen schema, bindings, actions and input |
| [Story rules](./rules.md) | WFT-RULES-001 | Headless typed conditions, deterministic events, ordered actions, limits and Phase 4 integration scope |
| [ADR 0008 — Declarative events](./adr/0008-declarative-events.md) | WFT-ADR-008 | JSONC event files, dialogue notification boundaries and save-safe rule inputs |
| [Phase 4 rules evidence](./evidence/phase-4.md) | WFT-EVID-009 | Automated condition/event/Story integration, candidate replay and rule-edit proof |
| [Story A author tutorial](./story-a-tutorial.md) | WFT-TUT-001 | Content-only dialogue/screen/theme/image edits and export |
| [Phase 3 Windows manual verification](./evidence/phase-3-windows.md) | WFT-EVID-008 | Native Story A/save/load steps and blank maintainer records |
| [Phase 3 implementation evidence](./evidence/phase-3.md) | WFT-EVID-007 | Save storage, checkpoints, player restoration and Story A automated checks |
| [Phase 0 evidence](./evidence/phase-0.md) | WFT-EVID-001 | Phase 0 check results and P0-10 deferral |
| [Phase 2 Windows manual verification](./evidence/phase-2-windows.md) | WFT-EVID-005 | Blank maintainer-owned native test records and steps |
| [Phase 2 Windows agent-run evidence](./evidence/phase-2-windows-agent.md) | WFT-EVID-006 | Agent-executed native window, frame, replay and perf observations; excludes audio and clean-machine checks |
| [Phase 2 implementation evidence](./evidence/phase-2.md) | WFT-EVID-004 | Compiled-dialogue slice and remaining Phase 2 checks |
| [Phase 1 evidence](./evidence/phase-1.md) | WFT-EVID-003 | Headless kernel checks and Windows CI evidence |
| [Phase 0 Windows evidence](./evidence/phase-0-windows.md) | WFT-EVID-002 | Windows verification artifacts and export evidence limits |

To add a document, copy the [template](./templates/document.md) and follow the [standards](./standards.md).

- [ADR 0007: safe checkpoint saves](./adr/0007-safe-checkpoint-saves.md)

- [ADR 0009 — Flat collection state](./adr/0009-flat-collection-state.md)

- [ADR 0010 — Bounded Rhai host](./adr/0010-bounded-rhai-host.md)

- [ADR 0011 — Canonical seeded randomness](./adr/0011-canonical-seeded-randomness.md)
