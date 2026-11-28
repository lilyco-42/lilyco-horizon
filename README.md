# Lilyco Horizon (`lilyco-horizon`)

Umbrella repository for the **Lilyco Horizon** family: the desktop Web Studio / Zero-UI Rust daemon, the shared YSM compiler, the five native mods, and the **Android (Starling)** and **OpenHarmony** client ports. The parts stay in their own repos as git submodules so each can fork, release, and be maintained independently.

## Layout

```
lilyco-horizon/                         this umbrella repo
├── crates/
│   ├── horizon-core/                   Horizon filesystem, YSM slot, schematic & mod manager
│   ├── horizon-i18n/                   external .toml locale editor & Horizon JSON sync
│   ├── horizon-daemon/                 Zero-UI Axum REST API (127.0.0.1:28787)
│   └── ysm-native-port/                shared Apache-2.0 .ytree/.yanim compiler & baker (22/22 tests)
├── apps/web-gui/                       Vite + React 19 Web Studio (http://127.0.0.1:5178)
├── clients/                            game client ports (git submodules)
│   ├── horizon-ohos/                   → Lyco-team/horizon-ohos-port (branch ohos-port)
│   │                                    OpenHarmony client: crates/horizon-ohos, touch UI, GL prewarm
│   └── horizon-android/                → Lyco-team/horizon-android-port (branch starling-v090)
│                                        Starling 0.9.0 Android baseline (starling-client/starling)
├── mods/                               native mods (git submodules)
│   ├── ysm_models/                     → lilyco-42/horizon-mod-ysm
│   ├── i18n_mod/                       → lilyco-42/horizon-mod-i18n
│   ├── schematics_mod/                 → lilyco-42/horizon-mod-schematics
│   ├── replay_mod/                     → lilyco-42/horizon-mod-replay
│   └── skins-3d_mod/                   → lilyco-42/horizon-mod-skins-3d
└── tools/build-umbrella.sh             unified build/status entry point
```

Clone with submodules:

```bash
git clone --recurse-submodules https://github.com/Lyco-team/lilyco-horizon.git
# already cloned?  git submodule update --init --recursive
```

The two client submodules are large (≈200 MB of source each). For a tools-only checkout skip them:

```bash
git clone https://github.com/Lyco-team/lilyco-horizon.git
git submodule update --init mods/
```

## Native Mods

| Mod | Version | Repository | Description |
| --- | --- | --- | --- |
| **`ysm_models`** | `4.0.2` | [lilyco-42/horizon-mod-ysm](https://github.com/lilyco-42/horizon-mod-ysm) | 1:1 Rust port of `YesSteveModel-Native`; 30-slot `.yanim` player, 3D held items, gestures and cross-fade |
| **`lilyco_i18n`** | `1.0.0` | [lilyco-42/horizon-mod-i18n](https://github.com/lilyco-42/horizon-mod-i18n) | external `.toml`-driven live ZH/EN localization switcher (`L` key + HUD) |
| **`schematics`** | `2.0.1` | [lilyco-42/horizon-mod-schematics](https://github.com/lilyco-42/horizon-mod-schematics) | Litematica-style 3D schematic projection, layer filter, hot-switch |
| **`replay_recorder`** | `3.0.1` | [lilyco-42/horizon-mod-replay](https://github.com/lilyco-42/horizon-mod-replay) | in-game replay & keyframe bookmark recorder (`R` / `B`) |
| **`skins_3d`** | `2.0.1` | [lilyco-42/horizon-mod-skins-3d](https://github.com/lilyco-42/horizon-mod-skins-3d) | 3D layered player skin renderer |

Each mod releases independently; this repo pins the known-good commit as a submodule.

## Client Ports

- **OpenHarmony** (`clients/horizon-ohos`, `Lyco-team/horizon-ohos-port`): Horizon client on `target_env=ohos`, built as `libhorizon_ohos.so` cdylib driven by an OpenHarmony Ability. Includes touch UI (virtual joystick, floating hub, OSK), wgpu GL adapter prewarm for cold-start watchdogs, and an outbound agent control channel. The HAP packaging project lives beside the client checkout and appends the game resources.
- **Android** (`clients/horizon-android`, `Lyco-team/horizon-android-port`): Starling 0.9.0 (`starling-client/starling`) baseline. Starling ships the full `horizon-mod-api`, so the desktop `ysm_models` source compiles against it unchanged; `tools/android_native_sdk.py mods` builds `.hmod` packages into `lib<id>.so` for the APK.

## Build

The umbrella does not force a single target. Use the subcommand you need:

```bash
./tools/build-umbrella.sh submodules   # init/update all submodules
./tools/build-umbrella.sh tools        # build the local workspace + test ysm-native-port
./tools/build-umbrella.sh mods         # list pinned mods and their build routes
./tools/build-umbrella.sh ohos         # OHOS client hints (needs the OHOS SDK)
./tools/build-umbrella.sh android      # Starling APK build hints (needs NDK 28 + cargo-ndk)
./tools/build-umbrella.sh status       # show all pinned submodule commits
```

Desktop tools and Web Studio:

```bash
cargo test --workspace
./start.sh
```

## Why submodules

The game clients and each mod are large, fast-moving, independently released codebases on different upstreams (Horizon official, Starling, per-mod repos). Keeping them as submodules lets this repo stay a small, auditable umbrella that pins exact known-good revisions, shares the `ysm-native-port` compiler, and gives one build entry point — without merging incompatible Cargo workspaces or forcing a 400 MB checkout on tools-only users.
