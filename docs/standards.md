---
id: WFT-STD-001
title: Weftora Documentation Standards
type: documentation_standard
doc_version: 0.1.3
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

# Documentation Standards

**Applies:** `PRD.md`, `ROADMAP.md`, authored `docs/*.md`, future doc content. **Exceptions:** root `README.md` (short entry point) and `docs/templates/*.md` (copyable templates). Docs-only policy; optional local checker available. No GitHub Actions workflow until future implementation phase.

## Mandatory YAML frontmatter

First bytes of each authored document must be `---`, followed by YAML map, closed with `---`. No content before header. Copy [template](./templates/document.md).

```yaml
---
id: WFT-ARCH-001
title: Weftora Architecture
type: architecture
doc_version: 0.5.1
status: proposed
implementation: not_started
created: 2026-10-08
updated: 2026-10-08
last_reviewed: null
owner: Weftora Maintainers
scope: windows-first
product_release: unreleased
related_pr: 1
supersedes: null
---
```

**Fields / types:**

| Field | Rule |
| --- | --- |
| `id` | Permanent, unique `WFT-[A-Z]+-NNN`; IDs do not follow filenames. |
| `title` | Human-readable and descriptive. |
| `type` | `product_requirements`, `roadmap`, `architecture`, `platform_plan`, `acceptance_criteria`, `docs_index`, `documentation_standard`, `architecture_decision`; extend deliberately. |
| `doc_version` | Quoted or scalar SemVer `MAJOR.MINOR.PATCH`; **independent from Engine/API/package/save versions**. |
| `status` | Exactly `draft`, `proposed`, `accepted`, `superseded`. |
| `implementation` | Exactly `not_started`, `in_progress`, `verified`, `not_applicable`. `verified` needs linked acceptance evidence. |
| `created` | Actual first document creation date; `YYYY-MM-DD`; immutable. |
| `updated` | Latest substantive or metadata revision date; `YYYY-MM-DD`, never before `created`. |
| `last_reviewed` | Last completed accountable review date or `null`. Proposal/PR creation is **not** review. |
| `owner` | Named team/role responsible for keeping doc current. |
| `scope` | `windows-first`, `multiplatform-planning`, `documentation`; add values deliberately. |
| `product_release` | Actual release/version/tag once issued; otherwise `unreleased`. |
| `related_pr` | GitHub PR number; `null` for docs without PR. |
| `supersedes` | Prior document ID or `null`; use on replacement, not routine file revisions. |

**YAML note:** Date-only scalars may parse as dates in YAML 1.1; tooling must normalize to ISO `YYYY-MM-DD` strings. Numeric `related_pr` stays integer; null values use literal `null`.

## Revision policy

- `doc_version` **PATCH**: metadata, links, typo, clarification with unchanged contract.
- **MINOR**: new nonbreaking requirements, details, examples, or stage success criteria.
- **MAJOR**: breaking architecture or public Story/Engine contract; require ADR and compatibility/migration analysis when implemented.
- Bump doc version on content/metadata change; update `updated` and add **Change History** row referencing PR.
- In one PR, update dependent docs and [index](./README.md) row versions/statuses/dates.
- Never reuse ID; if replacing doc, mark old `superseded`, link new ID through `supersedes`.
- `doc_version` tracks text only: never imply `engine_version`, `api_version`, `schema_version`, `save_format` or test success from a doc bump.

## Status and implementation

**Approval lifecycle:** `draft` → `proposed` (open for review) → `accepted` (explicit maintainer approval) → `superseded`. Revisions to accepted docs re-enter `proposed` when approval is required; don't overwrite approval history silently.

**Implementation lifecycle:** `not_started` → `in_progress` → `verified`. `not_applicable` for pure governance/index docs. A doc can be accepted while implementation remains `not_started`; being documented never means implemented.

For `implementation: verified`, include release/commit, P0–P7 criteria references, reproducible test logs and Windows environment where relevant; set `last_reviewed` to actual approval date. No fabricated completion.

## Expected document structure

1. YAML metadata.
2. H1 title, consistent with metadata.
3. Overview / goal and non-goals.
4. Scope, contracts, decisions or requirements.
5. Validation / acceptance references.
6. Open decisions or risks, where applicable.
7. **Change History** table: date, doc_version, short summary and PR/ADR link.

Not every section required for indexes or reference docs; metadata and Change History mandatory.

## Review checklist

- [ ] Header parses as YAML; exactly required keys with allowed types/values, unique ID and ISO dates.
- [ ] `created` preserved; `updated` not earlier than created; `last_reviewed` only actual review.
- [ ] `doc_version` increment matches change scope; Change History and index updated.
- [ ] No stale references, contradictory statuses or unsupported claims of implementation.
- [ ] Relative Markdown links resolve; related docs updated for contract changes.
- [ ] For technical changes, required roadmap tests/acceptance evidence defined; no PASS without tests.
- [ ] Docs metadata and content remain compatible with Windows-first foundation scope.

**Optional local validation:** run `python scripts/check_docs.py` manually before doc PR review. Checker validates frontmatter, IDs, enum/date values, document index and local Markdown links/anchors. **No GitHub Actions workflow in this PR**; automated docs/Cargo/Story/architecture gates are future Phase 0 tasks.

## Change History

| Date | Version | Change | Reference |
| --- | --- | --- | --- |
| 2026-10-08 | 0.1.3 | Remove GitHub Actions; retain optional local checker. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.1.2 | Add Windows docs checker and matching governance rules. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.1.1 | Register architecture decision docs and planned docs validation CI. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
| 2026-10-08 | 0.1.0 | Define Option B YAML metadata and review policy. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
