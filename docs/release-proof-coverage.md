---
id: WFT-REL-002
title: Release proof coverage
status: proposed
---

# Release proof coverage

[P6-03](./success-criteria.md#phase-6--windows-foundation-release) requires earlier SHA-256 and fixture checks on the release build. This table maps existing requirements to maintained commands and identifies gaps; it adds no requirements or waivers. Automation present in a workflow is not a passing candidate result. Historical Windows observations and local Mac debug reports do not establish a release rerun.

## Commands and retained evidence

The [Windows release job](../.github/workflows/ci.yml) uses the pinned toolchain and lockfile. It runs `cargo test-all --release --target x86_64-pc-windows-msvc -- --test-threads=1`, builds matching tools, then runs the maintained helpers with explicit release CLI/player paths and fresh outputs outside `target/`. Engine independence runs last because it can rebuild workspace artifacts.

| Earlier checks | Maintained release command/report | Coverage and remaining evidence |
| --- | --- | --- |
| P0-01 build/identity | Workspace release build; queried identities in helper reports | Automated build/profile/target/hash evidence; exact candidate rerun required |
| P0-02 dependency direction; P0-03 merge gate | Required CI `cargo xtask deps`, lint/docs/dependency checks; release Engine-independence report | Graph/policy checks are independent of optimization profile. Historical injected-failure gate proof is not repeated by a green run |
| P0-04 Yarn branches; P0-08 path refusals | Full release Rust suite | Automated fixtures; native walking-skeleton behavior remains separate |
| P0-05/P0-06 Windows display and interaction | No headless replacement | Native release observations remain in issue #5 |
| P0-07 actual Hello Story text edit | `verify-new-story.py`: `new-story-proof.json` → `helloTextEdit` and four Hello branch traces | Same player bytes validate both artifacts; CLI headless traces differ only by greeting. Compiled dialogue changes; native visual observation separate |
| P0-09/P0-11 format and GO decisions | Existing ADRs 0004/0001 | Decisions retained, not executable rerun claims |
| P0-10 clean-machine export | No automated substitute | Explicit historical deferral; clean Windows machine/VM still required |
| P1-01–P1-09 kernel, loader, compatibility and determinism | Full release Rust suite; default stripped-source suite from Engine-independence helper | Automated fixtures; retain logs from the selected candidate |
| P2-01/P2-02/P2-03/P2-06/P2-09 state, diagnostics, formats, UI and assets | Full release Rust suite; all-example export report; A/B future-artifact refusal | Automated/model coverage and compiler-free validation. Actual UI/native asset behavior is not inferred from these tests |
| P2-04/P2-05 views/audio; P2-08 performance | ECS/model tests run in release suite; no native/performance substitute | Native integration and measurements remain required |
| P2-07 pins/licence inventory; P2-10 renderer decision | Lockfile, required dependency CI, toolkit notice report and ADR 0005 | Pin/decision evidence retained; final notice/native-component audit remains open |
| P3-01/P3-03 playable Story A and navigation | Story A package/headless fixtures plus release model tests | Native completion/input/resize not established; historical evidence is in open PR #21 |
| P3-02 dialogue/screen/theme/image edits | `verify-story-a-edits.py`: `same-player-proof.json` | Same-player exports and text-patch old-save continuation; native before/after visuals remain separate |
| P3-04–P3-06 saves and compatibility | Release Rust suite; Story A and A/B helpers | Checkpoint/storage/refusal/replay fixtures; native quit/relaunch/view observation separate |
| P3-07 independent author walkthrough | No automated substitute | Someone other than the tutorial author must perform it |
| P4-01/P4-02/P4-04/P4-07 conditions/events/collections/RNG | Release Rust suite; `verify-mechanics-edits.py`: `same-player-proof.json` | Six guarded edits, exact mechanics traces, checkpoint replay and schema refusals |
| P4-03 commands; P4-06 screens | Mechanics helper; `verify-screen-edits.py`; release Rust suite | Same-player package/headless and UI-model coverage; native demonstrations remain separate |
| P4-05 Rhai host | Release hostile-script fixtures; mechanics helper | Implemented bounded host is exercised; script-internal Engine calls remain explicitly deferred under ADR 0010 |
| P5-01/P5-02 Story B and shared player | `verify-life-sim.py`: `same-player-proof.json` | A/B and pay-edit exports/headless completion; native Story B/shared-player completion remains open |
| P5-03 Engine independence | `verify-engine-independence.py --profile release --target x86_64-pc-windows-msvc --reuse-target` | Stripped A/B source, release build/default suite and retained player refusal; dedicated reports/logs |
| P5-04/P5-05 A/B matrices and versions | Release Rust suite and life-sim helper | Refusal/candidate/save preservation and compatible revisions. Modeled writer metadata is not a historical-binary guarantee |

The release artifact also retains `weftora-release-example-exports.json` for all 12 baseline Stories. ZIP, notice and combined-checksum reports are additional packaging evidence; they do not replace earlier gameplay or native checks. See [Phase 6 evidence](./evidence/phase-6.md) for historical runs and implementation limits.

## Actual changed-build mismatch

The [historical changed-source experiment](./evidence/phase-6.md#export-tool-identity-p6-02-automated-slice) used Mac debug tools. The maintained [tool-drift helper](../scripts/verify-tool-drift.py) now repeats its real-build controls in Windows release CI:

```text
python -B -O scripts/verify-tool-drift.py --cli target/x86_64-pc-windows-msvc/release/weftora.exe --player target/x86_64-pc-windows-msvc/release/weftora-player.exe --target x86_64-pc-windows-msvc --cache-target target --out <fresh-directory-outside-checkout>
```

The supplied pair must match. The helper first exports and validates A/B with that original pair. It copies the Engine source to its own workspace (omitting A/B, which are excluded from the tool fingerprint), while controls and refusals use the untouched original Stories. It builds into its own Cargo target, never the original target.

Optional `--cache-target` seeds dependency artifacts, host build dependencies and fingerprints by copying them into that isolated target. Incremental data, final top-level executables and test executables are excluded. Windows uses ordinary copies; macOS uses independent copy-on-write clones on the same filesystem. No hardlinks are used. This is a warm-cache proof, not a cold-build claim. Without the option, the isolated build starts without a seed.

Before the unchanged control build, the helper queries Cargo metadata and cleans every path package, including workspace and vendored crates, in host and selected-target layouts **only in the isolated target**. The control requires Cargo to report all path-crate artifacts freshly compiled; its identity must exactly equal the baseline. This prevents cached workspace libraries or build-script values from masquerading as a relocation control. The original and isolated matching pairs must both export/validate A/B.

It then appends the recorded comment to the isolated CLI source and rebuilds only the CLI. Cargo must report a real CLI rebuild and dependency info from the isolated checkout. The new identity must differ only in `buildId`. Both A/B Stories, with absent and existing-empty outputs, must refuse specifically with both build IDs in the mismatch diagnostic, exit 1, no stdout and unchanged source/output inventories. Original tool hashes and the complete original source inventory are checked again.

`tool-drift-proof.json` records the source delta, identities/hashes, commands, path-cache reset, four matching controls and four refusals. Build/refusal logs are retained too. Successful runs remove their own source/target copies; failures can leave partial output for inspection, without a success report. Never promote the rebuilt CLI as candidate bytes. Run from a quiescent checkout and do not run another Cargo build against the cache while seeding. The source, original build outputs and candidate exports remain unchanged.

The Windows job invokes this after candidate packaging and before Engine independence. Its report/logs are evidence, not download assets. New head CI and the exact selected main candidate must still run it successfully; automation being present is not acceptance.

## Remaining P6-03 acceptance

Run the mapped checks at the exact selected `main` candidate and retain its full SHA, target/profile, commands and reports. Require the real changed-build report above and review the table against the candidate artifacts. Native/manual rows stay open until their own release evidence is accepted through issue #5. P6-03 is not checked off by this document, and the foundation release remains gated by all of P6-01–P6-12.
