# Lilyco Horizon (`lilyco-horizon`)

Production-grade Rust & React suite for **Horizon (Minecraft)** — featuring a 1:1 Rust port of **Yes Steve Model (`YesSteveModel-Native` + `YesSteveModel`)**, 5 native Horizon mods, an externalized `.toml` i18n engine, and a Zero-UI REST daemon + Web Studio GUI.

## Included Horizon Native Mods (`mods/`)

| Mod | Version | Description |
| --- | --- | --- |
| **`ysm_models`** | `3.5.0` | Full 3D YSM hierarchical bone-tree renderer (`euler_zyx`, `PoseStack`, `ModelState`), 30-slot `.yanim` player (`idle / walk / run / sneak / jump / ride / swim / extra0..7 / arm.animation.json`), smooth cross-fade blending, live 3D held-item rendering (`ThirdPersonRightHand` / `ThirdPersonLeftHand`), and left-click swing overlay. |
| **`lilyco_i18n`** | `1.0.0` | External `.toml`-driven live Chinese/English localization mod (`L` key toggle + HUD indicator). |
| **`schematics`** | `2.0.1` | Litematica-style 3D schematic projection & hot-switch mod. |
| **`replay_recorder`** | `2.0.0` | In-game replay & keyframe marker recorder (`R` toggle / `B` bookmark). |
| **`skins_3d`** | `2.0.1` | High-detail 3D layered player skin renderer. |

## Workspace Crates & Apps

- `crates/ysm-native-port` — Apache-2.0 1:1 Rust port of `YesSteveModel-Native` (`euler`, `pose_stack`, `bone_attribute`, `model_state`, Bedrock `geo` parser with `(-X, +Y, -Z)` Euler convention, `molang` evaluator, 30-slot `.ytree`/`.yanim` baker with `QuadType::kEmpty` alpha culling). **12/12 unit tests passing.**
- `crates/horizon-core` — Zero-trust Horizon filesystem, YSM slot, Litematica schematic, and mod manager.
- `crates/horizon-i18n` — External `.toml` locale bundle editor & Horizon JSON sync engine.
- `crates/horizon-daemon` — Decoupled Zero-UI Axum REST API (`127.0.0.1:28787`) + `/api/v1/ai/capabilities`.
- `apps/web-gui` — Vite + React 19 + TypeScript + Tailwind v4 Web Studio (`http://127.0.0.1:5178`).
- `tools/build_lyco_from_cc0_default.py` — CC0 3-color (`60-30-10`) `lyco_ip` YSM model generator.

## Quick Start

### 1. Run Tests & Web Studio
```bash
cargo test --workspace
./start.sh
```

### 2. Build & Install Native Mods in Horizon
Copy `mods/*` into `~/.local/share/horizon/horizon-native-mods/` and compile with the official Horizon Launcher CLI:
```bash
horizon-launcher native-build \
  --version nightly-2026-11-20-0a875da \
  --trust-native-code \
  ~/.local/share/horizon/horizon-native-mods
```

### 3. In-Game Controls (`ysm_models` v3.5.0)
- `Y`: Cycle 3D YSM character model
- `G` / `/ysm_play <0..7>`: Cycle or jump to extra actions (`idle / wave / cheer / work / sit / dance / bow / victory`)
- `W/A/S/D`, `Shift`, `Ctrl`, `Space`, `Left Click`, `Mount Horse`: Automatic locomotion, sneaking, jumping, sword/item swinging, riding posture, and 3D held-item rendering

## Credits & Licenses
- Core workspace & mods: **MIT / Apache-2.0**
- YSM math & bone-tree architecture ported from [YesSteveModel/YesSteveModel-Native](https://github.com/YesSteveModel/YesSteveModel-Native) & [YesSteveModel/YesSteveModel](https://github.com/YesSteveModel/YesSteveModel) (Apache-2.0).
- Default `lyco_ip` model base derived from official YSM CC0 `builtin/default`.
