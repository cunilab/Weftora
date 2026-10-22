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

The container decision is [ADR 0013](./adr/0013-folder-release-packages.md). The Windows release-verification CI job builds/tests the release profile, verifies baseline exports of all 12 checked-in examples against one frozen player, and collects the maintained starter, Story edit, A/B and Engine-independence proofs. The folder transport packager is implemented and CI exercises it for standalone Story A, life-sim and localisation games. Authoring-tools candidate staging, a licence-text review inventory and combined download checksum staging are implemented. Draft preparation automation is implemented below. Final licence/advisory review, remaining evidence recipes and an actual main-candidate workflow run are still pending.

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

## Record candidate build provenance

Windows release CI initializes the installed Visual Studio developer shell for an x64 host/target, then runs:

```powershell
$linker = Join-Path $env:VCToolsInstallDir "bin/Hostx64/x64/link.exe"
python -B -O scripts/build-windows-candidate.py --linker $linker --out <fresh-directory-outside-checkout>
```

Use this recipe instead of the two standalone Cargo commands above when collecting build evidence. It owns the full locked `test-all` suite and workspace release build. The release job no longer restores Cargo build artifacts. The helper requires an absent `target/`, a clean checkout (including untracked/ignored files outside `target/`), and only the tracked workspace Cargo configuration; the listed Rust/Cargo/MSVC compiler, wrapper, flag and directory overrides refuse. Both Cargo commands explicitly select the target and intermediate build directory. This intentionally restricted recipe may refuse an ordinary developer checkout: use a fresh checkout rather than deleting unrelated local work.

`build-provenance.json` records the actual checkout commit/tree, working-tree source-file hashes, lockfile/toolchain/configuration hashes, resolved Rust compiler/rustdoc/Cargo hashes and verbose versions, explicit MSVC linker and sibling C compiler hashes, raw MSVC version output and probe return codes, selected VC/SDK settings, runner image and CI run/attempt. It records the resulting CLI/player hashes and hashes the retained logs and `source-files.json`. Only selected environment fields are copied; credentials and arbitrary environment/configuration values are not dumped. MSVC help/no-input version probes may return nonzero; their expected version banners are required and their return codes are retained. Actual test/build commands must succeed. Source files and recorded tools/configuration are checked again after the build. A fresh success report is renamed into place last; partial logs without it do not establish success.

CI uploads `weftora-windows-build-provenance-<workflow-commit-SHA>-attempt-<N>` immediately after the build step, including any partial logs on failure, so inspection can begin before the longer changed-build and Engine-independence proofs finish. Only a complete `build-provenance.json` establishes completion of this recipe. This early artifact does not establish that later CI steps passed. The final release-evidence artifact also uploads this build-folder path with the other proof reports. The SHA is the workflow checkout commit (a synthetic merge on PRs), and the attempt suffix distinguishes reruns. Retention is 14 days; preserve selected release evidence before expiry.

Before combined ZIP staging, CI compares the frozen tool hashes with this report. Keep the report and logs in the separate release-evidence artifact, alongside package/import evidence; they are not additional downloadable game assets. The checkout commit may be a PR merge commit, so retain the recorded GitHub event/ref/SHA separately and rerun the final selected `main` candidate. Only the selected executable tool files are hashed; toolchain DLLs and sysroot libraries are not fully inventoried. MSVC's C compiler is inventoried, not proven to be every native build script's selected compiler. The report is unsigned evidence, not a hermetic/reproducible-build or signed supply-chain attestation. It does not establish all native library inputs, a precise minimum Visual C++ Redistributable version, source-distribution compliance or clean-machine acceptance. Check the selected candidate's runtime against [Microsoft's guidance](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) during issue #5 testing.

The developer-shell arguments follow [Microsoft's documented x64 architecture and location controls](https://learn.microsoft.com/en-us/dotnet/framework/tools/developer-command-prompt-for-vs). Explicit output-directory selection follows [Cargo's configuration rules](https://doc.rust-lang.org/cargo/reference/config.html).

## Stage the authoring-tools candidate

Build the CLI/player together from the selected clean candidate first. Keep the same checkout and lockfile for notice collection; the helper checks matching tool identities and workspace versions, but cannot prove an old same-version pair was built from the current dependency graph. Then run:

```text
python -B -O scripts/stage-authoring-tools.py --cli target/x86_64-pc-windows-msvc/release/weftora.exe --player target/x86_64-pc-windows-msvc/release/weftora-player.exe --out <fresh-folder-outside-checkout>
```

The folder includes matching tools, a Windows author README, Weftora's existing MIT/Apache licence choices as full texts, `THIRD-PARTY-NOTICES.txt`, `dependency-notices.json`, `WINDOWS-REQUIREMENTS.txt` and copied upstream texts under `licenses/`. Source texts retain their bytes and attribution. The [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html) inventory starts at both apps, follows normal/build edges and excludes dev-only edges. It filters for the tool target and conservatively includes workspace-unified features/build tools; it is not an exact binary-linkage or embedded-asset inventory. Recursive collection includes named font/controller notices as well as package-root texts. The pinned upstream mapping also supplies the separately licensed embedded Fira Mono and player-interface Fira Sans font notices omitted from the `bevy_text` archive; see the [embedded-font audit](./evidence/phase-6.md#embedded-font-notice-audit). Missing texts/metadata are explicitly listed, never filled with guessed upstream attribution. The [checked-in upstream collection](../third-party/notices/README.md) supplies pinned fallback texts and provenance for exact package/version/source/SPDX/revision matches. Staging verifies every used file hash and refuses changed or dirty VCS provenance; it does not fetch notices. Policy-only documents and nested notices alone do not clear a missing package-licence flag.

The `licenses/` tree also includes the full published `smartstring` source archive, marked `source-archive` in the inventory. Its bytes must match the exact name/version/registry checksum in Cargo.lock as well as the checked-in index; changed or missing mappings refuse. The notice gives recipients a local source path, original declared terms, hash and `tar -xf` instructions. Paths are relative to the notice file, so the same text works under a game's `notices/` folder. Keep the entire tree, including `.crate` files, when redistributing the candidate. Staging neither downloads nor extracts source archives. Failed staging may leave partial files: do not package an unsuccessful run, and retry into a fresh folder. This version-bound archive does not prove which dependency source produced an old binary; build matching tools from the selected reviewed candidate first.

Every staged package is an **unapproved verification candidate**: `auditApproved` stays false even when all Cargo metadata rows have some text. The release audit must resolve missing upstream texts against pinned sources, review embedded assets/external native libraries and their terms, inspect any source-distribution obligations, and review the existing advisory exceptions. Cargo-deny passing its declared-licence policy does not complete this audit. The [example-resource/advisory review](./evidence/phase-6.md#example-resource-and-advisory-review) records preserved asset notices and current exception reachability. The [smartstring source packaging proof](./evidence/phase-6.md#smartstring-source-archive-and-recipient-notice) now supplies the published source archive and recipient notice; copying a licence file alone is not recorded as completion, and final source/build provenance and obligation review remain open. [MIT](https://opensource.org/license/mit) and [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) are the workspace's already-declared alternatives; third-party terms remain independent.

Package this folder using the ZIP transport helper, then run the maintained starter proof with the extracted tool pair. Windows CI stages, packages and exercises this candidate, retaining the dependency inventory and ZIP for review. This does not establish independent-author, native or clean-machine acceptance and cannot authorize publication.


## Inspect Windows native imports and prerequisites

Current MSVC candidate tools import `VCRUNTIME140.dll`. The package does not bundle a Visual C++ runtime installer or loose DLLs. `WINDOWS-REQUIREMENTS.txt` gives the official Microsoft v14 x64 Redistributable links and accompanies the toolkit and staged games. [Microsoft's guidance](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) requires a runtime at least as recent as the build tools. Record the selected candidate's toolchain/runtime versions and verify startup on the clean Windows test machine; a developer machine's installed runtime does not establish clean-machine acceptance.

After ZIP verification, inspect the extracted package folders with LLVM objdump, without launching the candidate executables:

```text
python -B -O scripts/audit-windows-imports.py --objdump <path-to-llvm-objdump> --package <extracted-toolkit-folder> --package <extracted-game-folder> --out <fresh-evidence-folder>
```

The helper scans every plain file's initial bytes, validates AMD64 PE signatures, records case-normalized ordinary DLL imports and retains raw LLVM private headers for each unique binary hash. Nonzero delay-import directories refuse because this inspection does not enumerate them. Linked inputs, unsupported headers/output, missing evidence and candidate changes refuse; the JSON report is written last. Do not package a failed run's partial evidence as successful. The inspector must be outside the candidate folders.

Windows release CI uses its installed LLVM objdump after all four ZIP extractions and before Engine cleanup; evidence is separate from download assets. The report records LLVM version, package inventory hashes and PE paths/hashes. It neither resolves transitive imports nor observes `LoadLibrary`, driver/backend selection, module versions on the target machine or redistribution rights. No system/SDK/runtime DLL is copied based on this report. See the [candidate evidence](./evidence/phase-6.md#windows-static-import-inventory-and-runtime-prerequisites). Native, clean-machine and final audit gates remain in issue #5.

## Include notices in standalone game candidates

Each game ZIP carries its own player and needs its own notice evidence. After export and validation, attach the matching toolkit's notices before packaging:

```text
python -B -O scripts/stage-game-notices.py --toolkit <staged-tools-folder> --game <exported-game-folder>
```

Run from the candidate checkout with the same lockfile used to stage the toolkit. The helper checks the export's CLI/player identities and CLI hash against the toolkit report, the actual game's player hash, the lockfile hash and the complete listed licence-file inventory. It copies only `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES.txt`, `dependency-notices.json`, `WINDOWS-REQUIREMENTS.txt` and `licenses/` under a fresh `notices/` directory. Byte checks and `notice-provenance.json` record the source report and player hashes; no CLI, compiler or toolkit README is copied. Existing notices, overlapping inputs, linked files and inventory/hash mismatches refuse. Use quiescent operator-controlled inputs; failed output may remain for inspection and must not be published. A missing `notice-provenance.json` means staging did not complete. Re-export into a fresh folder before retrying.

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

This helper verifies byte identity against the selected package checksums; it does not repeat transport extraction, authenticate the checksum author, establish a shared build identity or approve release contents. Keep the per-package transport reports and candidate-SHA evidence. Windows release CI retains a separate `weftora-windows-download-candidate-<SHA>-attempt-<N>` artifact with all four ZIPs, the combined checksums and inventory. Its artifact wrapper is for review; the contained ZIPs are the candidate download bytes. Later CI failure or any pending foundation criterion still prevents release approval.

After extracting the candidate artifact wrapper, release operators can check its contained download folder from a reviewed Engine checkout without writing or extracting anything:

```text
python -B -O scripts/stage-release-assets.py --verify <download-directory>
```

This mode requires exactly the ZIPs named by `release-assets.json`, plus that inventory and `SHA256SUMS`. It checks bounded plain metadata/files, record types, portable ZIP names, case/Unicode aliases, sorted checksum/inventory agreement, per-ZIP size and SHA-256, and unchanged metadata/files through the end of verification. Limits match staging (16 ZIPs, 2 GiB each, 4 GiB total); each metadata file is limited to 128 KiB. It rejects extra or missing entries, links/reparse points and mixing `--verify` with staging arguments. CI also runs it on the combined staged folder before upload. Failure never modifies the input folder.

This checks consistency and corruption, not authenticity: a matching inventory and ZIP can both have been replaced. Independently establish the selected source commit, successful CI run/attempt and trusted evidence before promoting bytes. This does not repeat ZIP extraction, establish binary provenance, approve licences or create a tag/draft/release. Use quiescent inputs controlled by the operator.

For a separate checksum-only check, run `sha256sum --check SHA256SUMS` where available. On Windows, compare each selected ZIP's `Get-FileHash -Algorithm SHA256` result with its entry. Use the exact staged paths in the draft-release recipe below, including the localisation demo when selected. Keep `release-assets.json` with the review evidence; the published `SHA256SUMS` covers ZIP downloads only.

## Release checklist

The [success criteria](./success-criteria.md#phase-6--windows-foundation-release) own the complete gate. The foundation release needs every criterion, including native and clean-machine evidence, even if its version is pre-1.0. Keep pending Windows records in [issue #5](https://github.com/cunilab/Weftora/issues/5); development can advance while publication waits for that evidence.

- [ ] Exact candidate SHA is on `main`, its version matches the workspace/lockfile, and required CI passes at that SHA.
- [ ] Windows release-profile build, all earlier SHA-256/fixture checks and Engine/Story independence have release-specific evidence (P6-03).
- [ ] Matching CLI/player identities, copied-player hashes, exports and clean-VM operation pass (P6-02). Keep the authoring pair together; each game example contains its own player and compiled dialogue.
- [ ] ZIP transport staging, path/link/bounds rejection fixtures and extracted-inventory/hash verification pass as required by ADR 0013. Publish SHA-256 checksums for the exact download assets.
- [ ] Native Windows matrix, independent author walkthrough and performance budgets pass (P6-04/P6-07/P6-08).
- [ ] Licence/notices and advisory audit, compatibility fixtures, debug tools and localisation requirements pass (P6-06/P6-09/P6-10/P6-11). Existing advisory exceptions must be reviewed explicitly.
- [ ] Release notes identify the full SHA, target/profile, tool identities/hashes, assets, evidence links, supported contracts and known limitations. Maintainer reviews the staged assets and authorizes publication (P6-12).

## Prepare a draft with verified CI artifacts

The manually dispatched `Prepare release draft` workflow runs only from `main` in this repository. Its default verifies inputs and retains a plan without creating a release. It needs an existing annotated version tag, a completed successful **push CI run from main** at that exact commit, and reviewed release notes. PR merge builds are ineligible. Create the tag using the reviewed recipe below only after the release checklist is satisfied.

The helper requires Python 3.11 or newer and authenticated GitHub CLI/API access. From a reviewed checkout containing the candidate and current remote main/tag objects, a local preflight is:

```sh
git fetch origin main --tags
python3.11 -I -B -O scripts/prepare-github-release.py \
  --tag v0.1.0 --run-id <completed-main-CI-run-ID> \
  --notes <reviewed-notes.md> --out <fresh-directory-outside-checkout>
```

It verifies tag/workspace/lockfile versions, ancestry on current remote main, the successful Windows/dependency/release-verification jobs and the current run attempt. It selects exact attempt-qualified build-provenance and download-candidate artifacts, checks their API SHA-256 digests, bounds downloads/extraction, compares the build's complete source manifest with raw Git blobs, and checks download checksums, tool identities, executable hashes and notice metadata. It refuses expired, ambiguous, mismatched or legacy artifacts. These new artifact names require this preparation change to merge and a fresh main push CI run; old PR evidence is not promoted. If CI needs a rerun, rerun all jobs so both artifacts and their provenance belong to the current attempt; partial reruns with only older artifacts refuse.

Known missing notice-text/metadata flags block preflight and draft creation, including the current `constgebra` gap. Passing these automatic checks does not approve licence obligations, advisory exceptions, native behavior or clean-machine acceptance. All release-checklist evidence still needs maintainer review; `auditApproved` remains false in retained records.

After completing that review, run the workflow from `main` with both `create_draft` and `checklist_reviewed` set to true, or add **both** `--create-draft --checklist-reviewed` to the local command using a fresh output directory. Creation rechecks remote inputs and local file hashes, requires the existing tag, and uploads only the four verified ZIPs plus `SHA256SUMS` to an unpublished draft with `--latest=false`. SemVer prereleases receive `--prerelease`. It then checks the draft's asset names, sizes and API digests before writing `draft-created.json`.

An existing draft or published release for that tag refuses; there is no overwrite, tag creation or publication path. A failed upload/readback can leave a partial draft: inspect it manually before any retry; the helper never deletes or repairs it. The workflow retains the plan, notes and successful readback record for 14 days. Inspect and approve publication separately using the recipe below. The workflow itself has not yet been dispatched against a qualifying main candidate.

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
