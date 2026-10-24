# Upstream notice evidence

`index.json` maps exact Cargo package versions, registry sources, declared licence expressions and published VCS commits to the upstream texts stored here. Source-text URLs use complete commit hashes; source archives use immutable versioned crates.io URLs plus their Cargo.lock checksum. The files retain upstream bytes, spelling, attribution and terms. Shared files may support several crates from the same repository revision. The stager verifies hashes and the crate's published VCS revision before copying them; collection needs no notice download during staging.

This collection supplies texts omitted from published crate archives. It also retains licensing-policy documents as `licensing-policy`: those documents remain useful evidence but do not clear a missing licence-text flag. Package-root texts and upstream fallbacks are tracked separately from nested third-party notices. Upstream `embedded-asset-notice` entries are copied with their provenance but cannot clear a missing package-licence flag. Each component remains subject to its own terms; the final redistribution audit remains open.

The checked-in corpus contains 54 package mappings and 47 unique upstream files. Tests verify every file hash, complete commit pin, supported source host and absence of unindexed files. Source archives are also checked against the exact registry package entry in Cargo.lock. Source selection follows the published `.cargo_vcs_info.json` revision and repository metadata. The older `gl_generator`/`khronos_api` archives omit `path_in_vcs`; their package subdirectories are recorded explicitly in the index. Preserve per-component licence variants such as MIT-0 and BSL-1.0.

## Remaining source-text gaps

| Component | Evidence at pinned source | Remaining work |
| --- | --- | --- |
| `constgebra 0.1.4` | The [published revision](https://github.com/knickish/constgebra/tree/eae8e094e5779f42f1db2a7adf9036aa33744bc8) has no licence text; Cargo declares `MIT OR Apache-2.0` | Resolve the missing text and attribution before release audit approval |
| `dispatch 0.2.0` (Mac development inventory) | The pinned repository tree has no licence text; Cargo declares `MIT` | Resolve separately for a future Mac distribution |
| Newer `madsmtm/objc2` crates (Mac development inventory) | The pinned `LICENSE.md` gives policy, links and SDK considerations | Retain the policy and review the actual terms/attribution; do not substitute an older commit's text |

The [release evidence](../../docs/evidence/phase-6.md#pinned-upstream-notice-texts) records current target inventories and checks. Embedded assets, external native libraries, source-distribution obligations and advisory exceptions still require release review. `auditApproved` remains false in every staged toolkit candidate.

When dependencies change, obtain source texts from that version's published revision, preserve their bytes and update the index/hashes in a reviewed PR. A missing mapping remains visible; source/SPDX/revision/hash changes for an existing mapping refuse staging. Add the new package/version's own evidence rather than reusing another version's attribution.

Standalone example games now receive a hash-checked copy of the matching toolkit notice evidence before ZIP staging; see [game notice staging](../../docs/releasing.md#include-notices-in-standalone-game-candidates). This preserves missing-text flags and does not close the unresolved-source or final audit gates.

## Embedded default font

`bevy_text 0.20.0` embeds `src/FiraMono-subset.ttf` with its enabled `default_font` feature. The font has separate OFL terms; Bevy's MIT/Apache texts alone do not describe those terms. The mapping includes the complete `assets/fonts/FiraMono-LICENSE` from Bevy's same pinned revision, retaining the Mozilla Foundation/Telefonica attribution. The packaged font bytes match that revision's subset, and its name table identifies Fira Mono Medium with OFL 1.1-or-later metadata. See the [embedded/native audit record](../../docs/evidence/phase-6.md#embedded-font-notice-audit) for hashes and scope.


## Included smartstring source

The collection includes the complete, unmodified published `smartstring 1.0.1` source archive under `bodil-smartstring/e407ca23c747257a812d2d2e70bf336412718c3a/`. Its 29,555 bytes have SHA-256 `3fb72c633efbaa2dd666986505016c32c3044395ceaf881518399d2f4127ee29`, matching Cargo.lock. The archive retains the source, original `LICENCE.md` and published VCS metadata; no source files were rewritten or repackaged.

The `source-archive` kind does not clear a missing package-licence flag. Staging requires matching package/source/SPDX/VCS provenance, archive bytes and lockfile checksum. An indexed source package changing version must get a new reviewed mapping rather than silently dropping its source archive. The existing `licenses/` inventory carries the archive into both toolkit and game packages, including ZIP round trips. `THIRD-PARTY-NOTICES.txt` gives recipients the local path, declared terms, SHA-256 and extraction instructions, relative to the notice's own folder. No network fetch or archive extraction is performed during staging.

This supplies the published version's source and a recipient notice. It does not attest that an old same-version binary was built from an unmodified registry source tree, or complete the final redistribution audit. Build the selected candidate from the reviewed clean sources and retain its evidence. See the [source distribution proof](../../docs/evidence/phase-6.md#smartstring-source-archive-and-recipient-notice).
