---
id: WFT-STD-001
title: Weftora Documentation Standards
status: proposed
---

# Weftora Documentation Standards

Applies to `PRD.md`, `ROADMAP.md` and everything under `docs/` except `docs/templates/`. The root `README.md` is a short entry point and has no header.

The goal is docs that stay correct with little effort. Git already records who changed what and when, so the docs do not duplicate that.

## Header

Every governed document starts with this YAML header and nothing before it:

```yaml
---
id: WFT-ARCH-001
title: Weftora Architecture
status: proposed
---
```

| Field | Rule |
| --- | --- |
| `id` | Permanent and unique, `WFT-<AREA>-NNN`. Never reused, even if the file moves. |
| `title` | Matches the H1 heading. |
| `status` | `draft`, `proposed`, `accepted` or `superseded`. |

No other fields. When a document replaces another, say so in its first paragraph and set the old one to `superseded`.

Status describes the **text**, not the software. Whether something is implemented is recorded only as test evidence against [success criteria](./success-criteria.md). A document never claims a feature works.

## One source of truth per topic

Each rule lives in exactly one document; others link to it instead of restating it.

| Topic | Owner |
| --- | --- |
| Product scope, users, requirements, non-goals | [PRD](../PRD.md) |
| Phase order and task lists | [Roadmap](../ROADMAP.md) |
| Crates, dependency rules, Story package, runtime semantics | [Architecture](./architecture.md) |
| Pass/fail checks and evidence | [Success criteria](./success-criteria.md) |
| Supported and future targets | [Platforms](./platforms.md) |
| Rationale for a hard-to-reverse choice | An [ADR](./adr/) |

If you find the same rule in two places, delete one and link to the other.

## Changing documents

- Status lifecycle: `draft` → `proposed` (open for review) → `accepted` (maintainer approved) → `superseded`. A substantive edit to an accepted doc puts it back to `proposed` until re-approved.
- Changes that alter a public contract (Story package format, Engine API, save format, dependency direction, renderer, scripting language) need an ADR or an ADR update in the same PR.
- New documents: copy the [template](./templates/document.md), pick a new ID, add a line to the [index](./README.md).
- Write plain English: full sentences, defined terms, no unexplained abbreviations.

## Review checklist

- [ ] Header has `id`, `title`, `status`; ID is unique.
- [ ] Relative links resolve.
- [ ] No rule duplicated from its owning document.
- [ ] No claim that something is implemented or passing without linked evidence.
- [ ] Contract changes carry an ADR.

Link and header checks become automated in Phase 0 CI (see [success criteria](./success-criteria.md)).
