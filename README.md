# Lilyco Horizon (`lilyco-horizon`)

Production-grade **Web Studio GUI & Zero-UI Rust Daemon** for **Horizon (Minecraft)** — managing YSM 3D models, Litematica schematics, external `.toml` i18n bundles, and Horizon native mods.

## Standalone Horizon Mod Repositories (One Repo per Mod)

Each Horizon native mod is maintained in its own focused repository so contributors can fork, open PRs, and release independently:

| Mod | Version | Repository | Release | Description |
| --- | --- | --- | --- | --- |
| **`ysm_models`** | `3.7.0` | [lilyco-42/horizon-mod-ysm](https://github.com/lilyco-42/horizon-mod-ysm) | [v3.7.0](https://github.com/lilyco-42/horizon-mod-ysm/releases/tag/v3.7.0) | 1:1 Rust port of `YesSteveModel-Native`, 30-slot `.yanim` player (`arm.animation.json` / `ride` / `extra0..7`), smooth cross-fade, real 3D held-item rendering, horse riding pose |
| **`lilyco_i18n`** | `1.0.0` | [lilyco-42/horizon-mod-i18n](https://github.com/lilyco-42/horizon-mod-i18n) | [v1.0.0](https://github.com/lilyco-42/horizon-mod-i18n/releases/tag/v1.0.0) | External `.toml`-driven live Chinese/English localization switcher (`L` key + HUD) |
| **`schematics`** | `2.0.1` | [lilyco-42/horizon-mod-schematics](https://github.com/lilyco-42/horizon-mod-schematics) | [v2.0.1](https://github.com/lilyco-42/horizon-mod-schematics/releases/tag/v2.0.1) | Litematica-style 3D schematic projection, layer filter, and hot-switch |
| **`replay_recorder`** | `2.0.0` | [lilyco-42/horizon-mod-replay](https://github.com/lilyco-42/horizon-mod-replay) | [v2.0.0](https://github.com/lilyco-42/horizon-mod-replay/releases/tag/v2.0.0) | In-game replay & keyframe bookmark recorder (`R` toggle / `B` bookmark) |
| **`skins_3d`** | `2.0.1` | [lilyco-42/horizon-mod-skins-3d](https://github.com/lilyco-42/horizon-mod-skins-3d) | [v2.0.1](https://github.com/lilyco-42/horizon-mod-skins-3d/releases/tag/v2.0.1) | 3D layered player skin renderer |

## What Lives in This Repo (`lilyco-horizon`)

- `crates/horizon-core` — Zero-trust Horizon filesystem, YSM slot, Litematica schematic, and mod manager.
- `crates/horizon-i18n` — External `.toml` locale bundle editor & Horizon JSON sync engine.
- `crates/horizon-daemon` — Decoupled Zero-UI Axum REST API (`127.0.0.1:28787`) + `/api/v1/ai/capabilities`.
- `crates/ysm-native-port` — Shared Apache-2.0 YSM `.ytree`/`.yanim` compiler & baker (22/22 tests).
- `apps/web-gui` — Vite + React 19 + TypeScript + Tailwind v4 Web Studio (`http://127.0.0.1:5178`).

## Quick Start
```bash
cargo test --workspace
./start.sh
```
