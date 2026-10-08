# Weftora Platform Support Plan

**Status:** proposed v0.1; documentation only, no verified platform builds.  
**Policy:** **Windows x64 first → full engine/story/authoring/export workflow → other OS.**  
Source of execution order: [ROADMAP.md](../ROADMAP.md). Core separation: [architecture](./architecture.md).

## 1. Target priority / support state

| Target | Rust target / runtime | Phase | Support status |
| --- | --- | --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` | Foundation (P0) | **Planned first; not implemented/tested** |
| Linux x64 | `x86_64-unknown-linux-gnu` (candidate) | After Windows (P1) | Deferred |
| macOS ARM64 | `aarch64-apple-darwin` | After Windows (P1) | Deferred |
| macOS Intel | `x86_64-apple-darwin` | Optional after macOS ARM64 | Deferred |
| Android ARM64 | `aarch64-linux-android` | Future (P2) | Deferred |
| iPhone/iPad ARM64 | `aarch64-apple-ios` | Future (P2) | Deferred |
| Web | `wasm32-unknown-unknown` + browser bridge | Future (P3) | Deferred |
| Windows ARM64 | `aarch64-pc-windows-msvc` (candidate) | Optional | Deferred |
| Steam Deck | Linux game/player + controller adaptation | Optional | Deferred |
| Consoles / TV | Platform-specific SDK & integrations | Research only | Not committed |

P0/P1/P2/P3 = **sequence**, not promised calendar, guaranteed platform support, or validated compatibility. Bevy/Rust target availability does **not** establish Weftora player support.

## 2. Windows foundation = full product, not partial tech demo

**Target:** `x86_64-pc-windows-msvc`. Choose minimum supported Windows version and Bevy graphics backend **after real Windows tests**. First release may use directory-style portable distribution; installer/signing optional based on distribution needs.

Planned outputs:
- `weftora.exe` — CLI with `new`, `check`, `run`, `export`.
- `weftora-player.exe` — compiled player loads external Story source/package, without Rust toolchain.
- Windows export — player + validated Story files, assets, and metadata; no dependency on developer machine absolute paths.
- Samples — VN Story A + distinct life-sim Story B, same player binary.

Windows test inventory:
1. Boot and exit; window resize, fullscreen/windowed, DPI scaling, common GPU/graphics issues.
2. Render background/characters/CG, dialogue, choices, custom UI, transitions and audio.
3. Keyboard/mouse, focus loss, accessibility basics and pause/resume; no hard-coded platform keycodes in Story logic.
4. External Story loader and export with spaces/Unicode in paths; reject traversal, missing/duplicate files; use relative content references.
5. Save/load at user-writable Windows path; safe checkpoints, save version mismatch and corrupt backups; verify saved data survives relaunch.
6. Headless flow, commands, state, scheduler, Yarn state mapping, fixture/golden tests + invalid-package cases.
7. Clean Windows x64 machine smoke test with **no Cargo/Rust SDK**; CLI exports standalone Story game; second Story runs using **identical compiled player**.
8. Validate license inventory and packaged third-party dependencies, startup errors/logs, documented prerequisites and basic performance/memory use.

**Windows completion gate:** all core acceptance tests above pass, Story A and B run unchanged Engine binary, author can change supported Story rules/content without Engine recompilation, and tagged Windows foundation release. **No other OS port before this gate.**

## 3. Multi-platform-ready architecture now (minimal cost)

Engine/core must remain independent from OS backend. Design narrow interfaces for platform-sensitive systems:

| Service contract | Engine owns | Platform adapter owns |
| --- | --- | --- |
| `AssetProvider` | Stable asset IDs and errors | Open bundled/disk/browser/mobile assets |
| `PlatformStorage` | Save scopes/atomicity requirements | User data path, file/DB operations, persistence |
| `PlatformInput` | Logical actions (confirm/cancel/skip) | Keyboard/mouse/touch/gamepad mapping |
| `RendererBackend` | Abstract view/layer/transition requests | GPU backend, swap chain, window/surface |
| `AudioBackend` | BGM/SFX/voice requests, volume semantics | Device/audio unlock and lifecycle |
| `PlatformLifecycle` | Play/pause/stop semantics | Suspend/resume, focus, OS callbacks |
| `PackageExporter` | Validate Story and target metadata | Bundle target player, resources/signatures |

Names are conceptual **contracts**, not committed Rust traits. Avoid gratuitous interfaces: implement only platform seams used by Windows, then adapt in later ports.

Portable Story principles:
- Story files use stable IDs and normalized relative references, never absolute host paths/drive letters.
- Save files external to read-only packaged content; platform locates durable writable storage.
- Localize text/assets once; platform builds may transcode/repackage assets but must preserve IDs and Story meaning.
- No Story logic depends on filesystem availability, physical file path separators, native windowing or OS-specific keycodes.
- Scripting/Yarn integrations must not expose unrestricted OS operations to Story.
- Core deterministic tests runnable headless, independent of input/render/audio stack.

## 4. Deferred ports: implementation checklist

These are **research/implementation candidates**, not guaranteed validated support.

### Linux x64 (P1)
Native executable; investigate distribution method (portable directory, AppImage, Flatpak), X11/Wayland, graphics driver/backend, audio, sandbox/file access, save locations, packaging and clean-machine tests. Steam Deck later through controller/layout/perf tests; do not equate desktop Linux build with Deck certification.

### macOS ARM64, then Intel if needed (P1)
macOS `.app` with architecture-specific Engine build + packaged Story. Validate Metal/GPU, Retina/high-DPI, input/audio, app bundle assets and OS save paths. Signing/notarization required for appropriate distribution channels. macOS build/signing typically needs Apple tools/runner; Windows CI cannot fully replace macOS hardware validation.

### Android ARM64 (P2)
Rust/Bevy Android backend integration, SDK/NDK and Gradle Activity/lifecycle bridge. Touch targets, display cutouts/safe areas, background/resume, mobile resource budgets, writable app storage, packaged read-only Story assets. Export `.apk` for local install and `.aab` for Play Store; verify store's **current target SDK, page-size/native-lib, signing and policy requirements when port begins**. Existing native Windows `weftora-player.exe` cannot be reused as Android binary.

### iOS / iPadOS ARM64 (P2)
Apple build chain/Xcode, signing/provisioning, app bundle/resources, sandbox storage, safe areas, touch and background lifecycle; test real devices + App Review constraints. Pack compatible Story with app or explicitly review permissible content updates. Dynamic interpreted code/content must be assessed under current App Store policy; do not promise unrestricted executable Story modding.

### WebAssembly / browser (P3)
Browser host/JS glue + `.wasm`, asset fetch and packaging, browser storage persistence, gesture-gated audio, browser file/security model, UI responsive layout and mobile browsers. Renderer backend may differ (WebGL2/WebGPU depending tested Bevy path). Do not assume native file IO or one-to-one desktop exporter logic; measure bundle size and memory limits.

### Optional later
Windows ARM64, Android TV, handheld controllers and consoles require separate build, testing and UX evaluation. Console support depends on SDK/access/publisher arrangements: no guarantee.

## 5. Gate to begin any new platform

All required before implementation:
1. Phase 6 Windows foundation exit signed off.
2. Freeze platform-independent Story schema and Engine API version (with explicit migration rules).
3. Select **one** next target; write ADR with tested Rust/Bevy/Yarn/platform toolchain version and fallback.
4. Define target player packaging, storage, lifecycle, input, renderer/audio and signing/distribution path.
5. Add target-specific CI + real-device smoke tests only once target work begins.
6. Re-run identical Story A/B fixtures and headless narrative/state/save tests; validate exported executable/package.
7. Advertise supported platform only after all tests pass; unsupported targets remain `experimental` or `unverified`.

**No blanket `cargo build --target` success claims.** Compile success alone does not prove export/playability.

## 6. Change control

- Do **not** add Android/iOS/Web abstractions with no Windows use case solely to predict future SDK needs.
- Do review code for OS assumptions and place Windows specifics behind backends. Avoid hardcoding OS logic in headless core or Story schema.
- Porting work cannot block Windows MVP. Any requested non-Windows build while P0 incomplete becomes future backlog item.
- Track new platform req changes here and in roadmap; PRD owns product acceptance; architecture owns module boundaries.
