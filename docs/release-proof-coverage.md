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

## Actual changed-build mismatch: release gap

The [historical changed-source experiment](./evidence/phase-6.md#export-tool-identity-p6-02-automated-slice) used real Mac **debug** tools. Release unit tests cover identity fields with responders, and the A/B helper refuses the actual CLI in the player role. Neither repeats a real changed-build release pair. The controls below repeat the [recorded experiment](./evidence/phase-6-tool-drift-macos.json). It still needs a release-specific rerun under P6-02/P6-03:

1. Retain the candidate's frozen player, CLI, identities and hashes; first prove the matching pair exports A/B successfully.
2. Use a separate checkout/worktree at the same full candidate SHA. Record a controlled CLI source-only change and before/after source hashes; do not modify the candidate checkout or published bytes.
3. Build only that checkout's CLI with the same pinned toolchain, lockfile, release profile and Windows target. Record its actual identity/hash. Require the build ID to differ while all other CLI identity fields remain equal. Keep the baseline player and prove its bytes unchanged.
4. Pair the changed CLI with the retained baseline player through `WEFTORA_PLAYER`. For both A/B Stories and both absent/existing-empty outputs, require the identity mismatch diagnostic, nonzero exit, no stdout and unchanged source/output inventories.
5. Retain commands, source delta, all identities/hashes, four refusal logs and matching-pair controls. The changed CLI is experiment evidence only; never promote it as a candidate tool.

Use isolated build outputs or prove the correct checkout was rebuilt; shared caches can retain fixture paths from another checkout. This recipe is documented, not installed or executed as release automation.

## Remaining P6-03 acceptance

Run the mapped checks at the exact selected `main` candidate and retain its full SHA, target/profile, commands and reports. Complete the real changed-build release experiment above, then review the table against the candidate artifacts. Native/manual rows stay open until their own release evidence is accepted through issue #5. P6-03 is not checked off by this document, and the foundation release remains gated by all of P6-01–P6-12.
