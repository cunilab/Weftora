---
id: WFT-DOCS-001
title: Weftora Documentation Index
type: docs_index
doc_version: 0.3.0
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

# Documentation Index

Single entry point for Weftora engineering/product docs. **Metadata in each file = source of truth**. Table below reflects PR #1 revision; update table in same PR as document metadata change.

## Current documents

| Document | ID | Doc version | Status | Implementation | Updated | Scope |
| --- | --- | --- | --- | --- | --- | --- |
| [Product Requirements](../PRD.md) | WFT-PRD-001 | 0.6.0 | proposed | not_started | 2026-10-08 | windows-first |
| [Roadmap](../ROADMAP.md) | WFT-ROADMAP-001 | 0.8.0 | proposed | not_started | 2026-10-08 | windows-first |
| [Architecture](./architecture.md) | WFT-ARCH-001 | 0.7.0 | proposed | not_started | 2026-10-08 | windows-first |
| [Platform Plan](./platforms.md) | WFT-PLAT-001 | 0.1.2 | proposed | not_started | 2026-10-08 | multiplatform-planning |
| [Success Criteria](./success-criteria.md) | WFT-ACCEPT-001 | 0.3.0 | proposed | not_started | 2026-10-08 | windows-first |
| [Documentation Standards](./standards.md) | WFT-STD-001 | 0.1.4 | proposed | not_applicable | 2026-10-08 | documentation |
| [Rust-native Engine ADR](./adr/0001-rust-native-engine.md) | WFT-ADR-001 | 0.1.0 | proposed | not_applicable | 2026-10-08 | documentation |
| [Versioning & Compatibility ADR](./adr/0002-versioning-compatibility.md) | WFT-ADR-002 | 0.1.0 | proposed | not_applicable | 2026-10-08 | documentation |

## Interpretation

- **Document status** = review/approval of text. **Implementation** = actual software verification. Neither implies other.
- **`product_release: unreleased`** until verified Weftora release; any example Engine/API/Story versions in ADRs or sample JSON are **illustrative only**, not actual published compatibility.
- **Windows x64 first:** platform plan covers future targets but does not mark them supported.
- **Review:** PR #1 contains proposed changes, not accepted/released specifications.

## Add/change documents

1. Copy [document template](./templates/document.md); assign unique stable ID.
2. Follow [documentation standards](./standards.md); update frontmatter, version and Change History.
3. Add/update index entry in same PR; check local links and status/implementation consistency.
4. If architecture/PRD/acceptance contract changes, update affected docs and capture review decision.

## Change History

| Date | Version | Change | Reference |
| --- | --- | --- | --- |
| 2026-10-08 | 0.3.0 | Register versioning ADR and revisions in PRD/roadmap/architecture/acceptance/platforms. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.2.3 | Align index after deferring local validation script. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.2.2 | Align index with removal of GitHub Actions. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.2.1 | Update docs checker status and standards revision. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.2.0 | Register new Rust decision ADR and updated docs revisions. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.1.1 | Track roadmap example-planning revision; no example files. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.1.0 | Introduce versioned doc register and index. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
