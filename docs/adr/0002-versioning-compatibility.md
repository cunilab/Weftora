---
id: WFT-ADR-002
title: Versioning and Compatibility Contract
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

# ADR 0002 — Versioning and compatibility contract

**Status: proposed, not implemented.** Approve contract in Phase 0 before locking first real manifest/state/API schemas. Versions below are **illustrative only**; no Weftora executable, public API, Story save format or dialogue artifact has shipped.

## Decision: independent version domains

| Domain | Field / identity | Policy |
| --- | --- | --- |
| Engine/player + creator tools release | `engineRelease`, build ID, binary SHA-256 | SemVer e.g. `0.3.0-alpha.1`. Identify exact tested CLI/compiler/player pair; patch release need not change public API |
| Story-facing Engine API | `engineApi` and manifest `requires.engineApi` | SemVer, independently from Engine release. `0.x` minor can break; from `1.0`, additive minor, fixes patch, breaks major |
| Story package manifest structure | `manifestSchema` | Integer, increment for incompatible parsing/required-field changes; deliberate backward support possible |
| Authored Story release | `storyVersion` | SemVer, independently from saved-state compatibility; content-only edits may increase patch |
| Engine save container serialization | `saveFormat` | Integer global format for all Stories; distinct from Story state schema |
| Story-owned persistent state and checkpoint semantics | `saveSchema` | Integer per `storyId`; bump for incompatible saved state or checkpoint contract |
| Compiled dialogue artifact | `dialogueArtifactFormat` | Integer for binary data contract; also record compiler ID/version and input hash; not Yarn source version |
| Doc versions + internal Rust dependencies | `doc_version`, Cargo crate versions | Independent; documentation changes do not imply runtime compatibility |

**No forced `1.0.0`** when Windows Phase 6 passes. `0.x` releases require explicit documented compatibility limits. For `requires.engineApi`, prefer explicit comparator ranges e.g. `>=0.2.0 <0.3.0`; pre-releases require explicit matching opt-in, not accidental acceptance of unstable versions. Define comparator semantics during P0; do not infer compatibility from matching major release alone.

## Proposed author Story manifest

```json
{
  "manifestSchema": 1,
  "id": "com.example.hello-story",
  "storyVersion": "1.2.1",
  "requires": {
    "engineApi": ">=0.2.0 <0.3.0",
    "capabilities": [
      "dialogue.yarn/v1",
      "ui.basic/v1",
      "flow.conditions/v1"
    ]
  },
  "saveSchema": 2,
  "entrypoint": "intro"
}
```

This is a **subset**; content catalogs and asset rules are described in [architecture](../architecture.md). Capability IDs include an explicit major contract revision; Engine may implement multiple versions. Requirement checks are inclusion tests against supported capabilities, not exact binary release comparison. An Engine API bump must not silently remove any capability promised within an advertised compatible range.

## Compatibility decisions, not release-number equality

1. **Story activation:** parse supported `manifestSchema` → validate `requires.engineApi` against current Engine API version, including pre-release policy → confirm versioned capabilities → check asset/data/command refs and compiled dialogue artifact format → activate atomically. Unsupported requirements fail before gameplay.
2. **Authoring vs shipping:** `weftora check` compiles Yarn source; `run` recompiles changed source; `export` packages compiled dialogue with artifact format, compiler ID/version, source IDs and content hash. Shipped player never needs author compiler; refuse unsupported artifact formats. Record CLI/compiler/player tested release build IDs and player SHA-256 so mixed releases cannot silently export an untested binary.
3. **Save load:** read bounded header with `saveFormat`, `storyId`, `storyVersion` (provenance), `saveSchema`, checkpoint ID, writer Engine API/release (provenance). Require **matching Story ID**, supported global format and compatible Story saveSchema + checkpoint. Writer API/release differences do **not alone** reject save; check current Story/runtime. Validate/migrate temp snapshot before atomic activation. Failure preserves existing in-memory state and known-good on-disk save.
4. **Story patch safety:** changing dialogue text, translation, sprites or audio with unchanged state schema and stable, semantically compatible checkpoint IDs must load older saves despite different `storyVersion`. Adding fields with deterministic defaults can remain same schema if backwards-compatible; semantic changes to choice/event resume points can break saves even if fields unchanged.
5. **Breaking Story changes:** removing/renaming saved fields or checkpoint IDs, or changing checkpoint meaning, requires new `saveSchema` plus explicit deterministic migration/mapping (or reject with diagnostics). A version bump alone does not migrate. Never silently start new game or overwrite save after refusal.
6. **Downgrades:** new Story/runtime saves are not guaranteed readable by older versions. Unsupported downgrade rejects; optional reverse migrations require their own tests. Migration chains must declare supported from/to schema and test repeated application/failure recovery.

## Compatibility fixtures required by phases

| Scenario | Expected |
| --- | --- |
| Engine release patch, public API unchanged | Same Story loads |
| Engine API additive within declared compatible range | Old Story loads unchanged |
| Engine API outside declared range or missing capability version | Reject with named requirement/versions |
| Unknown manifest schema or dialogue artifact format | Reject before activation; explain supported formats |
| Story text/assets-only patch, unchanged saveSchema/checkpoint | Old save loads |
| New optional saved field with deterministic default | Old save loads if declared schema-compatible |
| Deleted/renamed checkpoint or incompatible saved state | Explicit migration/mapping or refusal; original save preserved |
| Save from Story A in Story B | Reject |
| Failed/corrupt migration or unknown save container format | Reject without changing current state or good on-disk save |
| Unsupported Engine/Story downgrade or prerelease range | Reject with versions and corrective guidance |
| CLI/compiler/player release mismatch at export | Reject instead of creating untested package |

Maintain tested fixtures using at least **two released Story revisions** and **two Engine release/API identities** where applicable; record supported API/capability/format matrix in Windows release notes. A binary SHA proves identity, not compatibility by itself.

## Design boundaries and deferred decisions

- Phase 0 resolves exact field names/serialization schemas, comparator parser, prerelease range rules, capability registry and positive/negative fixtures **before implementation contract freezes**.
- Phase 2 resolves compiler artifact layout, cache invalidation, compiler/runtime adapter compatibility, supported Yarn subset and export metadata.
- Phase 3 resolves stable checkpoint representation, schema migration hooks, transactional activation and corruption recovery.
- Phase 6 publishes supported-version matrix and rejects untested tooling combinations. No auto-update service, package registry, complex migration framework or scripts required in this planning PR.
- Breaking changes to public fields/capabilities/persistence must update this ADR, PRD, architecture, roadmap and acceptance cases together.

## Change History

| Date | Version | Change | Reference |
| --- | --- | --- | --- |
| 2026-10-08 | 0.1.0 | Propose independent version domains, compatibility matrices and safe migrations. | [PR #1](https://github.com/cunilab/Weftora/pull/1) |
