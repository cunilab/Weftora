---
id: WFT-REL-001
title: Trunk-based development and releases
status: proposed
---

# Trunk-based development and releases

## Workflow

`main` is the trunk. Development uses small, short-lived branches and reviewed pull requests back to `main`, with the [merge gate](./architecture.md#10-drift-protection) passing. Prefer squash merging for one reviewable change per commit. Keep unfinished functionality out of release candidates. There is no permanent `develop` branch or routine release branch.

A release selects a verified commit already on `main`, creates an annotated version tag on that exact commit, and publishes its frozen artifacts through GitHub Releases. Development can continue on `main` while the selected commit is verified. A later `main` commit is a different candidate and needs its own evidence. This is the [release-from-trunk approach](https://trunkbaseddevelopment.com/branch-for-release/), using tags rather than a stabilization branch.

The existing long-running PR #19 remains open for maintainer review; this policy does not rewrite its history or merge it automatically. Future work should use smaller branches. Hotfixes follow the same trunk-first PR and new-tag flow. Supporting an older release line would need an explicit backport policy and CI; it is outside the foundation workflow.

## Current implementation boundary

This document records the intended release process. Current CI runs on PRs and pushes to `main`; no tag-triggered release builder or publisher is implemented yet. Pushing a tag currently does not build or publish release assets. Debug previews and release-verification artifacts are development evidence, not approved foundation release assets.

The container decision is [ADR 0013](./adr/0013-folder-release-packages.md). The Windows release-verification CI job builds/tests the release profile, verifies baseline exports of all 12 checked-in examples against one frozen player, and collects the maintained starter, Story edit, A/B and Engine-independence proofs. The folder transport packager is implemented and CI exercises it for standalone Story A, life-sim and localisation games. Authoring-tools candidate staging, a licence-text review inventory and combined download checksum staging are implemented. Final licence/advisory review, remaining evidence recipes and draft-release publishing automation are still pending.

## Prepare a candidate

1. Choose the Engine release SemVer in a normal PR. Update the workspace package version, lockfile, compatibility documentation and release notes together. Do not bump the independent Engine API or Story save schema merely because the Engine release changes; [ADR 0002](./adr/0002-versioning-compatibility.md) owns those rules.
2. Merge the reviewed preparation PR after required CI passes. Record its full `main` commit SHA as the candidate, along with the version and CI/evidence links. A branch name or moving `main` tip is not a build input.
3. Build in a clean detached checkout/worktree at that SHA on Windows x64 with the pinned toolchain and lockfile. An older, verified `main` commit is eligible; confirm it is reachable from `origin/main`. Keep all staging/proof directories outside Cargo `target/` and start with fresh destinations.
4. Freeze the matching release CLI/player pair and build inputs. If executable signing is introduced, sign before final hashing, export and acceptance tests. Promote the tested bytes; do not rebuild or alter executables after recording their hashes.

The relevant release-profile commands in that candidate checkout are:

```text
cargo build --workspace --release --locked --target x86_64-pc-windows-msvc
cargo test-all --release --target x86_64-pc-windows-msvc -- --test-threads=1
```

Both tools then come from `target/x86_64-pc-windows-msvc/release/`. Run proofs with these explicit `--cli` and `--player` paths and fresh `--out` directories. Never combine a debug CLI with a release player. The existing starter, Story A edit, screen edit, A/B same-player and six-case mechanics-edit scripts accept explicit tool paths. Run Engine independence with `python -B -O scripts/verify-engine-independence.py --profile release --target x86_64-pc-windows-msvc --reuse-target --out <fresh-directory-outside-checkout>`. It applies the selected profile/target to inventory, build, tests, missing-fixture controls and workspace cache cleanup, and records them in the report. The default remains `--profile dev` with the host target. This proof rebuilds workspace artifacts; run it after retaining the frozen tool/export proofs. The [all-example baseline report](./evidence/phase-6.md#all-example-baseline-exports-p6-03-automated-slice) records copied-player and compiled-dialogue hashes plus compiler-free validation for every checked-in Story. It requires the exact maintained example list, freezes input tool hashes and verifies them again after the loop, then retains `weftora-release-example-exports.json`. The [isolated mechanics-edit helper](./evidence/phase-6.md#isolated-mechanics-edit-recipes-p6-03-automated-slice) now automates the earlier rule/command/collection/script/seed/query edits, exact checkpoint replay and schema refusals with those same frozen tools. Run `python -B -O scripts/verify-mechanics-edits.py --cli <matching-cli> --player <matching-player> --out <fresh-directory-outside-checkout>`. A failed run can leave partial output without `same-player-proof.json`; never approve it. Windows release CI retains its report, traces and refusal logs. The [release proof coverage table](./release-proof-coverage.md) maps earlier requirements to commands/reports and records the real changed-build experiment, now automated by `verify-tool-drift.py` in Windows release CI using isolated source and target directories. The starter helper now also repeats the actual Hello Story text edit on both branches. Final candidate reruns and native/manual evidence remain required. These two Cargo commands alone do not satisfy P6-03.

## Stage the authoring-tools candidate

Build the CLI/player together from the selected clean candidate first. Keep the same checkout and lockfile for notice collection; the helper checks matching tool identities and workspace versions, but cannot prove an old same-version pair was built from the current dependency graph. Then run:

```text
python -B -O scripts/stage-authoring-tools.py --cli target/x86_64-pc-windows-msvc/release/weftora.exe --player target/x86_64-pc-windows-msvc/release/weftora-player.exe --out <fresh-folder-outside-checkout>
```

The folder includes matching tools, a Windows author README, Weftora's existing MIT/Apache licence choices as full texts, `THIRD-PARTY-NOTICES.txt`, `dependency-notices.json` and copied upstream texts under `licenses/`. Source texts retain their bytes and attribution. The [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html) inventory starts at both apps, follows normal/build edges and excludes dev-only edges. It filters for the tool target and conservatively includes workspace-unified features/build tools; it is not an exact binary-linkage or embedded-asset inventory. Recursive collection includes named font/controller notices as well as package-root texts. The pinned upstream mapping also supplies the separately licensed embedded Fira Mono font notice omitted from the `bevy_text` archive; see the [embedded-font audit](./evidence/phase-6.md#embedded-font-notice-audit). Missing texts/metadata are explicitly listed, never filled with guessed upstream attribution. The [checked-in upstream collection](../third-party/notices/README.md) supplies pinned fallback texts and provenance for exact package/version/source/SPDX/revision matches. Staging verifies every used file hash and refuses changed or dirty VCS provenance; it does not fetch notices. Policy-only documents and nested notices alone do not clear a missing package-licence flag.

Every staged package is an **unapproved verification candidate**: `auditApproved` stays false even when all Cargo metadata rows have some text. The release audit must resolve missing upstream texts against pinned sources, review embedded assets/external native libraries and their terms, inspect any source-distribution obligations, and review the existing advisory exceptions. Cargo-deny passing its declared-licence policy does not complete this audit. The [example-resource/advisory review](./evidence/phase-6.md#example-resource-and-advisory-review) records preserved asset notices and current exception reachability. It also identifies the remaining `smartstring` MPL source-availability/recipient-notice work; copying a licence file alone is not recorded as completion. [MIT](https://opensource.org/license/mit) and [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) are the workspace's already-declared alternatives; third-party terms remain independent.

Package this folder using the ZIP transport helper, then run the maintained starter proof with the extracted tool pair. Windows CI stages, packages and exercises this candidate, retaining the dependency inventory and ZIP for review. This does not establish independent-author, native or clean-machine acceptance and cannot authorize publication.


## Include notices in standalone game candidates

Each game ZIP carries its own player and needs its own notice evidence. After export and validation, attach the matching toolkit's notices before packaging:

```text
python -B -O scripts/stage-game-notices.py --toolkit <staged-tools-folder> --game <exported-game-folder>
```

Run from the candidate checkout with the same lockfile used to stage the toolkit. The helper checks the export's CLI/player identities and CLI hash against the toolkit report, the actual game's player hash, the lockfile hash and the complete listed licence-file inventory. It copies only `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES.txt`, `dependency-notices.json` and `licenses/` under a fresh `notices/` directory. Byte checks and `notice-provenance.json` record the source report and player hashes; no CLI, compiler or toolkit README is copied. Existing notices, overlapping inputs, linked files and inventory/hash mismatches refuse. Use quiescent operator-controlled inputs; failed output may remain for inspection and must not be published. A missing `notice-provenance.json` means staging did not complete. Re-export into a fresh folder before retrying.

This is the matching toolkit's conservative normal/build dependency superset, including authoring-only entries; it does not imply those components are linked into the player. Missing-text flags and `auditApproved: false` are retained. Story content, fonts/media, external native libraries, attribution and redistribution obligations still require their own review. The localisation Story's original font/OFL/provenance remain under `story/fonts/`. The `constgebra 0.1.4` text gap remains unresolved; copying its metadata does not supply missing terms.

Windows release CI applies this step to all three example games, then validates the extracted player again and checks that notices are present without an authoring CLI. Raw `weftora export` does not automatically attach Engine notices: authors sharing other exports must copy the matching toolkit's notice files and `licenses/` tree into a `notices/` folder too, alongside their own asset/content notices. Weftora's licence choice covers the Engine, not independently authored Story content. Notice presence does not complete P6-06 or authorize publication.

## Verify ZIP transport

After exporting and validating a game folder with the frozen tools, package that reviewed folder with:

```text
python -B -O scripts/package-folder.py --source <reviewed-folder> --out <fresh-transport-directory>
```

Use a quiescent source and output parent controlled by the release operator. This is a release-build helper for its own generated archive, not a public arbitrary-ZIP extraction service. It snapshots the selected folder inventory, writes `<folder-name>.zip`, extracts it under `verified/<folder-name>/`, and compares every file name, size and SHA-256 plus empty directories and executable bits. It checks the source inventory again before emitting `folder-inventory.json`, `transport-proof.json` and `SHA256SUMS`. A failure leaves partial output for inspection; rerun into a fresh destination. Never publish outputs from a failed command.

The fixed per-package bounds are 10,000 entries including directories/root and 1 GiB of uncompressed file data. The packager refuses links, junctions/reparse points, special files, traversal/absolute paths, Windows-reserved names, case/Unicode aliases, duplicate/unexpected ZIP members and mismatched types/sizes/hashes. Windows naming follows the conservative reserved-name checks in [CPython's Windows path implementation](https://github.com/python/cpython/blob/3.13/Lib/ntpath.py#L282-L306). No archive support is added to the Engine.

Run the extracted player with the extracted Story and `--validate` before using these bytes for native/clean-machine acceptance. Windows release CI does this for fresh Story A, life-sim and localisation exports and retains the ZIPs, inventories, reports and checksums; its GitHub artifact wrapper is not itself a release asset. Stage the authoring toolkit first and attach its matching notice evidence to each standalone game as described above. Packaging a folder does not approve its contents or complete the licence audit.

## Stage combined download checksums

After all selected folders pass their transport round trips and extracted-tool/Story checks, collect their ZIPs into one fresh download directory:

```text
python -B -O scripts/stage-release-assets.py --package <tools-transport-directory> --package <story-a-transport-directory> --package <life-sim-transport-directory> --package <localization-transport-directory> --out <fresh-download-directory>
```

Each `--package` selects a transport output directory containing exactly the one ZIP named by its `SHA256SUMS`. The stager checks the source hash before and after copying, verifies the copied bytes, then writes a sorted combined `SHA256SUMS` and `release-assets.json` with names, sizes, hashes and source-directory basenames as labels. It refuses duplicate case/Unicode aliases, unsafe names, linked inputs, changed ZIPs/checksums, existing destinations and outputs inside an input package. Limits are 16 packages, 2 GiB per ZIP, 4 GiB total and 4 KiB per input checksum file. Use quiescent inputs and an output parent controlled by the operator; failure can leave partial output, which must not be published. Rerun into a fresh directory.

This helper verifies byte identity against the selected package checksums; it does not repeat transport extraction, authenticate the checksum author, establish a shared build identity or approve release contents. Keep the per-package transport reports and candidate-SHA evidence. Windows release CI retains a separate `weftora-windows-download-candidate-<SHA>` artifact with all four ZIPs, the combined checksums and inventory. Its artifact wrapper is for review; the contained ZIPs are the candidate download bytes. Later CI failure or any pending foundation criterion still prevents release approval.

For independent verification after extracting the artifact wrapper, run `sha256sum --check SHA256SUMS` where available. On Windows, compare each selected ZIP's `Get-FileHash -Algorithm SHA256` result with its entry. Use the exact staged paths in the draft-release recipe below, including the localisation demo when selected. Keep `release-assets.json` with the review evidence; the published `SHA256SUMS` covers ZIP downloads only.

## Release checklist

The [success criteria](./success-criteria.md#phase-6--windows-foundation-release) own the complete gate. The foundation release needs every criterion, including native and clean-machine evidence, even if its version is pre-1.0. Keep pending Windows records in [issue #5](https://github.com/cunilab/Weftora/issues/5); development can advance while publication waits for that evidence.

- [ ] Exact candidate SHA is on `main`, its version matches the workspace/lockfile, and required CI passes at that SHA.
- [ ] Windows release-profile build, all earlier SHA-256/fixture checks and Engine/Story independence have release-specific evidence (P6-03).
- [ ] Matching CLI/player identities, copied-player hashes, exports and clean-VM operation pass (P6-02). Keep the authoring pair together; each game example contains its own player and compiled dialogue.
- [ ] ZIP transport staging, path/link/bounds rejection fixtures and extracted-inventory/hash verification pass as required by ADR 0013. Publish SHA-256 checksums for the exact download assets.
- [ ] Native Windows matrix, independent author walkthrough and performance budgets pass (P6-04/P6-07/P6-08).
- [ ] Licence/notices and advisory audit, compatibility fixtures, debug tools and localisation requirements pass (P6-06/P6-09/P6-10/P6-11). Existing advisory exceptions must be reviewed explicitly.
- [ ] Release notes identify the full SHA, target/profile, tool identities/hashes, assets, evidence links, supported contracts and known limitations. Maintainer reviews the staged assets and authorizes publication (P6-12).

## Tag and stage a GitHub Release

The commands below are an operator recipe after the checklist passes, not automation installed by this document. Replace the quoted placeholders with the reviewed values and exact staged asset paths. Use PowerShell on the release host:

```powershell
$candidateSha = '<approved-full-main-commit-SHA>'
$version = '<approved-SemVer>'
$tag = "v$version"
$notes = '<path-to-reviewed-release-notes.md>'
$assets = @('<path-to-tools.zip>', '<path-to-story-a.zip>', '<path-to-life-sim.zip>', '<path-to-localization-demo.zip>', '<path-to-SHA256SUMS>')
git fetch origin main --tags
if ($LASTEXITCODE -ne 0) { throw 'Fetch failed' }
git merge-base --is-ancestor $candidateSha origin/main
if ($LASTEXITCODE -ne 0) { throw 'Candidate is not on main' }
git tag -a $tag $candidateSha -m "Weftora $tag"
if ($LASTEXITCODE -ne 0) { throw 'Tag creation failed' }
git push origin "refs/tags/$tag"
if ($LASTEXITCODE -ne 0) { throw 'Tag push failed' }
gh release create $tag @assets --verify-tag --draft --latest=false --title "Weftora $tag" --notes-file $notes
if ($LASTEXITCODE -ne 0) { throw 'Draft release creation failed' }
```

[Annotated tags](https://git-scm.com/docs/git-tag) identify releases; add tag signing when the project's signing policy is configured. Treat published tag names and asset bytes as immutable. Never force-move a release tag or replace published bytes; corrections get a new version and a new verification run.

[`gh release create --verify-tag`](https://cli.github.com/manual/gh_release_create) requires the remote tag to exist, avoiding automatic tagging of a moving default branch. `--draft` stages the release for inspection. For a SemVer prerelease such as `v0.1.0-rc.1`, also pass `--prerelease` and keep `--latest=false`; a prerelease label does not waive the foundation checklist.

## Publish and fix forward

Inspect the draft's tag target, uploaded filenames, downloaded asset checksums, notes and evidence. After maintainer approval, publish a normal release with:

```powershell
gh release edit $tag --draft=false --latest
```

For a prerelease use `gh release edit $tag --draft=false --prerelease --latest=false` instead. These [GitHub CLI options](https://cli.github.com/manual/gh_release_edit) publish the staged release; no source commit or asset rebuild belongs between verification and publication.

Fix a released defect on a new branch from current `main`, add a regression check, merge through review, then choose and verify a new candidate/version. A release tag is a record of tested source and bytes, not a branch to continue developing on. If staging fails before publication, repair the draft process against the same candidate or select a new candidate; never silently point an existing version at different code.
