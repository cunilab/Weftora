# Weftora — Roadmap Success Criteria

**Status:** proposed, docs-only. **Authoritative roadmap:** [ROADMAP.md](../ROADMAP.md). **Initial target:** Windows x64 (`x86_64-pc-windows-msvc`). **Future targets:** [platforms.md](./platforms.md).

## Acceptance rules

A phase is **PASS** only if **every mandatory criterion** passes and repeatable evidence is linked in its PR, release checklist or CI run. `TODO` / code compiling / screenshot alone is **not** completion. Any failed/untested mandatory criterion = **BLOCKED**; no inferred PASS. A deliberately dropped requirement needs a reviewed PRD + roadmap change, not an unchecked exception.

- **Evidence per criterion:** test ID, git commit SHA, pinned toolchain/dependency versions, command or manual procedure, platform/OS/device, input fixture, expected vs actual output, result, and log/artifact link. Windows graphics/audio/manual tests need OS build, GPU and driver versions.
- **Automation:** Rust unit/integration, CLI acceptance, manifest fixtures and headless golden tests. Use CI run IDs and captured stdout/stderr. Manual gameplay/device checks need recorded test steps/results; include video/screenshots when useful, never as sole proof of functional correctness.
- **Binary identity:** record SHA-256 of `weftora-player.exe` before and after Story edits and across sample Stories. Equal hash proves same artifact; also verify each package runs successfully. Do not compare exports that intentionally include different Story assets as whole-directory hashes.
- **Negative behavior:** invalid input must yield documented error/nonzero exit (CLI) or controlled player error, not panic/silent success/partial Story activation. Schema/API incompatibility rejects **before** gameplay.
- **Versions:** Story manifest, Engine API and save format versioning independent; compatibility/migration outcomes tested explicitly.
- **Regression:** all earlier phases' mandatory tests continue passing on current Engine head. A new phase cannot silently invalidate prior acceptance.
- **Windows-first:** Phase 0–6 tests/builds on Windows x64 only. Keep platform-independent Rust core interfaces without creating Linux/macOS/mobile/Web implementation or CI. Phase 7 unavailable until Phase 6 passes.

## Phase 0 — Contract + Rust workspace

**Outcome:** headless Rust workspace + validated Story contract; **no gameplay expected yet**.

| ID | Mandatory success criterion | Verification / proof |
| --- | --- | --- |
| P0-01 | Workspace contains `weftora-api`, `weftora-core`, `weftora-story`, CLI/player stubs; `cargo metadata` resolves; pinned `rust-toolchain.toml` and committed `Cargo.lock` | `cargo metadata --locked --format-version 1`, `cargo check --workspace --locked --target x86_64-pc-windows-msvc`; CI green |
| P0-02 | Headless crates have **no direct or transitive** `bevy`, Yarn, Rhai or example Story crate deps; Engine never imports game-specific types | Machine-checkable `cargo metadata` dependency-graph assertion + workspace unit test; sample Story directory not a Cargo package |
| P0-03 | Manifest schema/API range, required capability, entrypoint and stable ID policies specified in versioned schema; valid fixture loads; malformed version/ID/path/manifest rejected with location and reason | Positive fixture + **at least** one negative fixture per validation class; tests capture expected diagnostic code |
| P0-04 | Windows x64 release target, provisional minimum OS support, version/license policy and dependency constraints documented; no unexplained `unsafe` in headless domain crates | ADR, pinned dependency inventory, automated `#![forbid(unsafe_code)]` policy where applicable |
| P0-05 | CI blocks failures in formatting, clippy, tests, manifest validation and architecture graph check | Clean CI run on fresh checkout; injected violation demonstrably fails at least one gate |

**PASS evidence:** Windows CI job, version/schema fixture test report, dependency graph report, ADRs. **Not enough:** empty crate stubs with no manifest tests.

## Phase 1 — Headless kernel

**Outcome:** deterministic state/command/signal runtime used by external Story manifest; no screen/renderer or complete flow scheduler yet.

| ID | Mandatory success criterion | Verification / proof |
| --- | --- | --- |
| P1-01 | Bootstrap → load → ready → shutdown invokes registered services in documented order, teardown in reverse, and failed init leaves no half-active Story | Lifecycle fixture with ordered trace; injected failed initialization and cleanup assertion |
| P1-02 | State `Get/Set/Add/Remove/Exists`, defaults and change notifications behave for bool/int/finite float/string; invalid type, missing key and nonfinite numeric value handled explicitly | Unit/property tests: mutation, notification counts, serialization-friendly semantics and expected errors |
| P1-03 | Registered generic command accepts typed args and returns success/error; async operation completes or cancels predictably; unknown command/bad args are rejected | Integration test of success, invalid args, cancellation and cleanup |
| P1-04 | Signals delivered with documented deterministic ordering; subscribe/unsubscribe works; recursion/loop limit prevents unbounded dispatch | Golden trace for nested emits and loop fixture ending in explicit bounded error |
| P1-05 | External Story catalog/manifest registers initial commands/content IDs; duplicate/missing ID diagnostics; `weftora check` and headless `run` skeletons use same loader | CLI fixture for valid + invalid Story. **Full Story-authored event scheduling deferred to P4** |
| P1-06 | Same input command/signal sequence generates **byte-identical normalized state + event trace** for 10 fresh runs on same Windows toolchain | Golden fixtures stored in repo; 10/10 comparisons match; no unstable timestamps/random order in normalized output |

**PASS evidence:** headless test log and 10-run golden diff. **Not enough:** compiled interfaces without executable command/state tests.

## Phase 2 — Windows Bevy + Yarn adapter feasibility

**Outcome:** confirm backend interoperability **before** building full VN.

| ID | Mandatory success criterion | Verification / proof |
| --- | --- | --- |
| P2-01 | Windows x64 player starts/exits cleanly, displays background sprite + text/choice UI, accepts keyboard/mouse, plays audible sample, loads assets through stable IDs | Manual smoke checklist + logs on recorded Windows OS/GPU/audio device; closed/relaunched successfully |
| P2-02 | Yarn source compiles/loads; narrative starts node, shows lines, offers choice(s), follows selected branch, reaches deterministic end | Fixture `intro.yarn` with at least two branches; captured line/choice/branch trace compared against expected |
| P2-03 | Yarn command reads/writes **canonical Engine state** (or explicitly synchronized mapping) without conflicting shadow values | Integration fixture: choice changes state, command reads it, next dialogue condition sees same value; wrong type reports diagnostic |
| P2-04 | Invalid Yarn syntax, missing node and unknown Story command fail with useful file/node context; no process panic | Three negative fixtures; expected error IDs and non-silent failure |
| P2-05 | Bevy/Yarn adapters stay outside headless crates; tested compatible version/feature set locked; unresolved upstream gaps documented | `cargo metadata` graph check, reproducible Windows build, ADR with known limitations and license inventory |
| P2-06 | If Rust Yarn port cannot meet P2-02–04, **replacement adapter** must meet same dialogue contract before phase can PASS | Comparative spike report and passing replacement tests; merely documenting failure **does not** pass phase |

**PASS evidence:** runnable Windows smoke sample, Yarn fixture traces, adapter tests, version ADR. **Not enough:** opening a window without executable Yarn dialogue.

## Phase 3 — First playable VN + safe persistence

**Outcome:** user plays complete short Story A through **prebuilt player**, no per-game Rust source.

| ID | Mandatory success criterion | Verification / proof |
| --- | --- | --- |
| P3-01 | Story A launches from external folder through `weftora-player.exe`; has menu, two dialogue branches/endings or outcomes, background, character image, CG, audio, visual transition, choice UI | Recorded scripted walkthrough for each branch, interactive Windows player test and asset manifest |
| P3-02 | Story-owned screen definitions, theme, sprite and Yarn lines change without modifying/rebuilding player | Edit external Story, validate/relaunch, compare pre/post player SHA-256: **same hash**, new content visible |
| P3-03 | Keyboard/mouse choices and menu navigation work; windows resize with readable UI; screen bindings reflect Engine state | Input/UI smoke steps on real Windows system with state assertions |
| P3-04 | One canonical gameplay state is shared across Yarn and Engine; no stale values after choice/command/update | Integration test: Yarn → Engine → Yarn round-trip and type-error case |
| P3-05 | Save at documented **safe checkpoint**, quit process, relaunch, load: Story ID/version, canonical state, choice-dependent outcome and reconstructed view match before-save expectation | Automated round-trip fixture + Windows player smoke. No promise of saving mid-async transition |
| P3-06 | Save uses user-writable Windows location, not immutable package assets; corrupt/incompatible save rejected with clear error and no state corruption | Negative save fixtures + clean save can still load after bad save attempt |

**PASS evidence:** player build SHA-256, Story A branch walkthroughs, two successive content edits proving no recompile, checkpoint restore tests. **Not enough:** demo running only from Cargo project.

## Phase 4 — Story-defined gameplay rules + configurable UI

**Outcome:** new rules/events/screens authored in Story files, not new Rust Engine gameplay code.

| ID | Mandatory success criterion | Verification / proof |
| --- | --- | --- |
| P4-01 | Story-defined comparison/boolean conditions (==, !=, >, >=, <, <=, AND/OR/NOT) evaluate typed values and reject invalid operands | Table-driven valid/invalid test fixtures for **every** operator |
| P4-02 | Trigger → condition → ordered actions runs with declared priority/tie-break; repeat/circular events terminate with explicit recursion/budget error | Golden event trace incl. competing priorities, async continuation/cancel and cycle case |
| P4-03 | Two new Story-specific gameplay actions (e.g. time advance, buy item) and one conditional event added through declarative data/registered exposed API, **without editing/rebuilding Engine** | Package diff only, same player SHA-256, state + signal outcomes asserted |
| P4-04 | Story may replace screen layout, theme, navigation and state binding through supported schema; invalid widget/action/binding rejected during validation | Before/after screenshots + UI interaction tests + invalid-screen fixtures |
| P4-05 | Decide optional Rhai with ADR: **not adopted** if declarative features suffice; **if adopted**, bounded execution/host-call allowlist, target compatibility, error paths and hostile-input tests must pass | Approved decision ADR and tests for selected path; no unsupported claim of safe untrusted mod execution |

**PASS evidence:** headless flow suite, Story-only diff, binary hash, Rhai ADR. **Not enough:** gameplay rule implemented as hardcoded Rust module.

## Phase 5 — Second Story demonstrates reuse

**Outcome:** same player runs two **meaningfully different** games.

| ID | Mandatory success criterion | Verification / proof |
| --- | --- | --- |
| P5-01 | Story B is life-sim-like (at minimum: two locations, state-driven schedule/trigger, resource change, conditional activity and custom screen) authored as separate external Story package | Story B fixture + recorded scenario showing all distinct systems |
| P5-02 | Same `weftora-player.exe` SHA-256 launches and completes Story A + B; no game-specific compiled Rust crate, no Engine source edits/rebuild between launches | Record one binary hash, launch commands and full smoke outputs for both packages |
| P5-03 | Removing/renaming both Story folders does **not** break Engine crate compilation; each Story can be replaced without source dependency | Headless `cargo check` and unit tests with Story fixtures isolated; runtime missing-package error is controlled |
| P5-04 | Rejected: duplicate IDs, missing assets/node/command, incompatible manifest/API, wrong Story save, bad schema, invalid UI and malformed paths; no partial activation | Negative fixture matrix; explicit diagnostic and failure-state assertions |
| P5-05 | Save from A cannot silently load in B; failed package load does not overwrite prior good state/saves; compatible migration case succeeds if migration feature shipped, otherwise rejects clearly | Cross-Story save + failure recovery tests |

**PASS evidence:** one binary hash + both Story logs, isolation build check, negative test matrix. **Not enough:** second Story is cosmetic variation of VN.

## Phase 6 — Windows foundation release (hard blocker)

**Outcome:** usable standalone Windows authoring/runtime product; **only then** future ports allowed.

| ID | Mandatory success criterion | Verification / proof |
| --- | --- | --- |
| P6-01 | Prebuilt `weftora.exe` supports `new`, `check`, `run`, `export`; new Story scaffolds valid content; invalid `check` fails with nonzero exit + actionable diagnostic | CLI integration suite with example invocation, assertions and exit codes |
| P6-02 | `export` produces standalone Windows game directory (or documented package) with player + Story + assets and **no author Cargo/Rust installation** | Export fixture, verify files/manifest, launch on clean Windows x64 machine/VM without Rust toolchain or source checkout |
| P6-03 | Story A/B use same compiled player; edits to rule/dialogue/UI/assets change outcomes without binary changes | Repeat P3/P4/P5 hash and fixture checks against **release build**, not debug binary |
| P6-04 | Full Windows player integration: dialogue branches, CG, UI/nav, audio channels, keyboard/mouse, resize/DPI, focus pause/resume, no crash on tested configuration | Signed-off real Windows test matrix incl. GPU/audio/OS versions and reproducible steps |
| P6-05 | Save/restart/load, version policy, incompatible/corrupt save handling, backup/atomicity, user data path all pass; invalid Story/asset/Yarn/UI errors surfaced with locations | Automated save and validator negative suites plus manual Windows relaunch |
| P6-06 | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, dependency-direction checks and licensing/advisory policies pass under pinned Windows toolchain | Green CI with exact command logs. If extra packages/tools required, document versions and commands |
| P6-07 | External creator unfamiliar with Rust uses published instructions to scaffold, edit, preview and export complete Story without Engine/Cargo code | Reproducible fresh-machine walkthrough (not maintainer's dev machine), written observed results + issues fixed |
| P6-08 | Windows target minimum OS/graphics prerequisites, known limitations, sample licenses/assets, Story/API/save format versions, migration/refusal policy and support instructions published | Release notes, dependency/license inventory, docs links, versioned binary/manifest evidence |
| P6-09 | No open **blocking** failures in earlier gates; Windows foundation release artifact tagged only **after** P6-01..08 pass | Explicit release checklist with reviewed PASS status, CI artifacts and binary hash |

**PASS evidence:** tagged Windows release link, clean-VM launch, published CLI/player binaries, two sample game runs, regression suite, checklist. **BLOCKED if any mandatory gate unverified.** Passing core tests alone does not unlock non-Windows development.

## Phase 7 — Per-platform port (locked until P6 PASS)

Phase 7 is **not a single all-platform PASS**. Each platform gets its own target-specific checklist/ADR, starting **after** Windows foundation release.

| ID | Mandatory success criterion **for each selected platform** | Verification / proof |
| --- | --- | --- |
| P7-01 | P6 Windows foundation marked PASS; select exactly one new target and document ABI/OS/toolchain/renderer/audio/asset/export/signing constraints | Linked Windows release gate + target ADR |
| P7-02 | Target-specific player runs Story A and B without changing **Story schema, core semantics or game rules**; platform-specific resource transcode permitted | Full existing headless golden fixtures + real native player test on target device |
| P7-03 | Target-specific input, scalable UI, audio, graphics, focus/suspend/resume, asset loading and durable save behavior pass | Device/browser test matrix and logs: desktop X11/Wayland/Metal, mobile touch/activity, Web audio/storage as relevant |
| P7-04 | Export/bundling/install/publish workflow succeeds for target; signing/notarization/store requirements satisfied **if distribution channel needs them** | Clean target installation + exported game smoke evidence |
| P7-05 | Incompatible Story/save package handling, permissions/path security and recoverable errors pass on target | Negative fixtures and integration logs; no silent state loss |
| P7-06 | New target regression passes **without breaking Windows release and headless core invariants** | CI results for Windows + new target, plus actual-device execution |
| P7-07 | Only completed target is marked **supported**; other targets remain planned/experimental | Per-target signed-off checklist + documentation update |

Targets in [platforms.md](./platforms.md): Linux x64, macOS ARM64/Intel, Android ARM64, iOS ARM64, WebAssembly; optional Windows ARM64/Steam Deck/TV/consoles later. Runtime shipping depends on platform access, policies, tooling and target-specific evidence, **not** Rust compiler target availability.

## Example evidence record

~~~yaml
criterion: P3-05
status: PASS # PASS | FAIL | BLOCKED | NOT_RUN
commit: "<git commit SHA>"
engine_binary_sha256: "<SHA-256 of weftora-player.exe>"
platform: "Windows x64; exact OS build + hardware"
toolchain: "<rustc / cargo / renderer / Yarn versions>"
scenario: "Story A: choose option B, checkpoint, exit, relaunch, load"
command_or_steps: "<automated command or manual test procedure>"
expected: "restored choice-dependent state and matching visible scene"
actual: "<observed output>"
evidence: "<CI artifact / log / screenshots / issue / test file>"
~~~

**No criteria should be marked PASS from docs-only work.** Implement tests, capture artifacts and record outcomes during development.
