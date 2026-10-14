---
id: WFT-PLAT-001
title: Weftora Platform Plan
status: proposed
---

# Weftora Platform Plan

## 1. Policy

- **Release target:** Windows x64 (`x86_64-pc-windows-msvc`) is the only platform Weftora releases for, supports or signs off phase gates on until the Phase 6 foundation release.
- **Development hosts:** maintainers may build, run and debug on macOS or Linux. These builds carry **no support claim**, need no release packaging, and must never block Windows work. Every gate that mentions the player, export or performance is verified on Windows.
- **Portable core:** headless crates contain no OS-specific code, and Story formats contain no OS-specific paths or keycodes. That keeps future ports cheap without building them early.

## 2. Targets

Port waves happen only after Phase 6 and are sequenced, not scheduled.

| Target | Rust target | When | Status |
| --- | --- | --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` | Foundation (Phases 0–6) | Planned release target |
| macOS / Linux desktop | host target | Phases 0–6 | Development host only, unsupported |
| Linux x64 | `x86_64-unknown-linux-gnu` | Port wave 1 | Deferred |
| macOS ARM64 | `aarch64-apple-darwin` | Port wave 1 | Deferred |
| Android ARM64 | `aarch64-linux-android` | Port wave 2 | Deferred |
| iOS / iPadOS ARM64 | `aarch64-apple-ios` | Port wave 2 | Deferred |
| Web | `wasm32-unknown-unknown` | Port wave 3 | Deferred |
| Windows ARM64, Steam Deck, consoles | various | Optional | Not committed |

A Rust target compiling does **not** mean Weftora supports that platform.

## 3. CI during the foundation

| Job | Windows x64 | Dev host (macOS/Linux) |
| --- | --- | --- |
| Headless crates: merge gate ([architecture §10](./architecture.md#10-drift-protection)) | Required | Recommended, non-blocking (catches OS assumptions early) |
| Player and adapters build | Required | Optional, non-blocking |
| Player smoke, export, performance | Required, on Windows | Not run |

## 4. Windows foundation test inventory

1. Start and exit; resize, fullscreen/windowed, DPI scaling.
2. Backgrounds, characters, CG, dialogue, choices, custom UI, transitions, audio.
3. Keyboard and mouse, focus loss, pause/resume.
4. Story loading and export with spaces and Unicode in paths; the path policy in [architecture §4](./architecture.md#4-story-package).
5. Saves in the user data directory (for example `%LOCALAPPDATA%\Weftora\saves\<storyId>\`); survive relaunch; corrupt and incompatible saves handled.
6. Clean Windows machine with no Rust or Cargo: CLI exports a game, both sample Stories run on the same player binary.
7. Licence inventory of shipped dependencies; startup errors and logs; documented prerequisites.

## 5. Platform seams

These are conceptual contracts, not committed Rust traits. Implement only what Windows needs, keeping OS details behind them.

| Seam | Engine side | Platform side |
| --- | --- | --- |
| Asset access | Stable IDs, errors | Disk, bundle, browser or mobile asset storage |
| Storage | Save scopes, atomicity requirements | User data path, file or browser storage |
| Input | Logical actions (confirm, cancel, skip, menu) | Keyboard, mouse, touch, gamepad mapping |
| Rendering | View, layer and transition requests | GPU backend, window or surface |
| Audio | Channel requests, volume semantics | Device and audio-unlock lifecycle |
| Lifecycle | Play, pause, stop | Suspend/resume, focus, OS callbacks |
| Export | Validated Story, target metadata | Player binary, bundling, signing |

Do not add abstractions for Android, iOS or Web with no Windows use.

## 6. Starting a port (Phase 7)

All of these are required before port work begins:

1. Phase 6 Windows foundation passed.
2. Story-facing contracts (manifest, Engine API, capabilities, save format, dialogue artifact) are versioned per [ADR 0002](./adr/0002-versioning-compatibility.md). Player binaries differ per platform; Story semantics do not.
3. Exactly one target chosen, with an ADR recording toolchain, renderer, packaging, storage, input, lifecycle and signing.
4. Target CI and real-device tests are added.
5. Story A and B, and all headless golden tests, pass unchanged on the target.
6. The platform is advertised as supported only after its Phase 7 checks pass.

### Port notes

- **Linux x64:** distribution format (portable folder, AppImage, Flatpak), X11/Wayland, audio, save location. A Linux build is not Steam Deck certification.
- **macOS ARM64:** `.app` bundle, Metal, Retina, signing and notarisation; needs Apple hardware for validation.
- **Android:** NDK/Gradle activity bridge, touch, safe areas, background/resume, scoped storage, AAB/APK; verify current Play Store requirements when the port starts.
- **iOS / iPadOS:** Xcode signing, app lifecycle, touch, safe areas, bundled Story content; check current App Store rules on interpreted content (Rhai scripts) before promising downloadable Stories.
- **Web:** wasm + JavaScript glue, fetched assets, browser storage, gesture-gated audio, WebGL2/WebGPU, memory and bundle size limits.
