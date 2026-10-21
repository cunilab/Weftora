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

The container decision is [ADR 0013](./adr/0013-folder-release-packages.md). The Windows release-verification CI job builds/tests the release profile and collects the maintained starter, Story edit, A/B and Engine-independence proofs. The folder transport packager is implemented and CI now exercises it for standalone A/B games. The authoring-tools licence inventory, remaining evidence recipes and draft-release publishing workflow with explicit promotion are still pending.

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

Both tools then come from `target/x86_64-pc-windows-msvc/release/`. Run proofs with these explicit `--cli` and `--player` paths and fresh `--out` directories. Never combine a debug CLI with a release player. The existing starter, Story A edit, screen edit and A/B same-player scripts accept explicit tool paths. Run Engine independence with `python -B -O scripts/verify-engine-independence.py --profile release --target x86_64-pc-windows-msvc --reuse-target --out <fresh-directory-outside-checkout>`. It applies the selected profile/target to inventory, build, tests, missing-fixture controls and workspace cache cleanup, and records them in the report. The default remains `--profile dev` with the host target. This proof rebuilds workspace artifacts; run it after retaining the frozen tool/export proofs. Earlier hash/fixture proof recipes must also be rerun or ported to maintained release automation; these two Cargo commands alone do not satisfy P6-03.

## Verify ZIP transport

After exporting and validating a game folder with the frozen tools, package that reviewed folder with:

```text
python -B -O scripts/package-folder.py --source <reviewed-folder> --out <fresh-transport-directory>
```

Use a quiescent source and output parent controlled by the release operator. This is a release-build helper for its own generated archive, not a public arbitrary-ZIP extraction service. It snapshots the selected folder inventory, writes `<folder-name>.zip`, extracts it under `verified/<folder-name>/`, and compares every file name, size and SHA-256 plus empty directories and executable bits. It checks the source inventory again before emitting `folder-inventory.json`, `transport-proof.json` and `SHA256SUMS`. A failure leaves partial output for inspection; rerun into a fresh destination. Never publish outputs from a failed command.

The fixed per-package bounds are 10,000 entries including directories/root and 1 GiB of uncompressed file data. The packager refuses links, junctions/reparse points, special files, traversal/absolute paths, Windows-reserved names, case/Unicode aliases, duplicate/unexpected ZIP members and mismatched types/sizes/hashes. Windows naming follows the conservative reserved-name checks in [CPython's Windows path implementation](https://github.com/python/cpython/blob/3.13/Lib/ntpath.py#L282-L306). No archive support is added to the Engine.

Run the extracted player with the extracted Story and `--validate` before using these bytes for native/clean-machine acceptance. Windows release CI does this for fresh Story A/B exports and retains the ZIPs, inventories, reports and checksums; its GitHub artifact wrapper is not itself a release asset. Authoring-tools staging must first add the matching tool pair, README and reviewed notices from ADR 0013. Packaging a folder does not approve its contents or complete the licence audit.

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
$assets = @('<path-to-tools.zip>', '<path-to-story-a.zip>', '<path-to-story-b.zip>', '<path-to-SHA256SUMS>')
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
