# Upstream notice evidence

`index.json` maps exact Cargo package versions, registry sources, declared licence expressions and published VCS commits to the upstream texts stored here. Source URLs use complete commit hashes. The files retain upstream bytes, spelling, attribution and terms. Shared files may support several crates from the same repository revision. The stager verifies hashes and the crate's published VCS revision before copying them; collection needs no notice download during staging.

This collection supplies texts omitted from published crate archives. It also retains licensing-policy documents as `licensing-policy`: those documents remain useful evidence but do not clear a missing licence-text flag. Package-root texts and upstream fallbacks are tracked separately from nested third-party notices. Each component remains subject to its own terms; the final redistribution audit remains open.

The checked-in corpus contains 52 package mappings and 45 unique upstream files. Tests verify every file hash, complete commit pin, supported source host and absence of unindexed files. Source selection follows the published `.cargo_vcs_info.json` revision and repository metadata. The older `gl_generator`/`khronos_api` archives omit `path_in_vcs`; their package subdirectories are recorded explicitly in the index. Preserve per-component licence variants such as MIT-0 and BSL-1.0.

## Remaining source-text gaps

| Component | Evidence at pinned source | Remaining work |
| --- | --- | --- |
| `constgebra 0.1.4` | The pinned repository tree has no licence text; Cargo declares `MIT OR Apache-2.0` | Resolve the missing text and attribution before release audit approval |
| `dispatch 0.2.0` (Mac development inventory) | The pinned repository tree has no licence text; Cargo declares `MIT` | Resolve separately for a future Mac distribution |
| Newer `madsmtm/objc2` crates (Mac development inventory) | The pinned `LICENSE.md` gives policy, links and SDK considerations | Retain the policy and review the actual terms/attribution; do not substitute an older commit's text |

The [release evidence](../../docs/evidence/phase-6.md#pinned-upstream-notice-texts) records current target inventories and checks. Embedded assets, external native libraries, source-distribution obligations and advisory exceptions still require release review. `auditApproved` remains false in every staged toolkit candidate.

When dependencies change, obtain source texts from that version's published revision, preserve their bytes and update the index/hashes in a reviewed PR. A missing mapping remains visible; source/SPDX/revision/hash changes for an existing mapping refuse staging. Add the new package/version's own evidence rather than reusing another version's attribution.
