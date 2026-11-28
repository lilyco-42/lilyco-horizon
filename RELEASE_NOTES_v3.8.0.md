# lilyco-horizon v3.8.0

Horizon（Minecraft Rust Edition）原生 mod 全家桶 + Web Studio 主仓库首次 Release，与各独立 mod 仓库版本对齐。

## 本仓库内容

- `crates/horizon-core` — Zero-trust Horizon 文件系统、YSM 槽位、Litematica 图纸、mod 管理器
- `crates/horizon-i18n` — 外置 `.toml` 语言包编辑器 + Horizon JSON 同步引擎
- `crates/horizon-daemon` — Zero-UI Axum REST API（127.0.0.1:28787）+ `/api/v1/ai/capabilities`
- `crates/ysm-native-port` — 共享 YSM `.ytree`/`.yanim` 编译器与烘焙器（Apache-2.0，22/22 测试）
- `apps/web-gui` — Vite + React 19 + TypeScript + Tailwind v4 Web Studio（127.0.0.1:5178）

## 各 mod 独立仓库与安装包（.hmod）

| Mod | 版本 | 仓库 | 下载 |
|---|---|---|---|
| ysm_models | v3.8.0 | https://github.com/lilyco-42/horizon-mod-ysm | https://github.com/lilyco-42/horizon-mod-ysm/releases/latest |
| lilyco_i18n | v1.0.0 | https://github.com/lilyco-42/horizon-mod-i18n | https://github.com/lilyco-42/horizon-mod-i18n/releases/latest |
| schematics | v2.0.1 | https://github.com/lilyco-42/horizon-mod-schematics | https://github.com/lilyco-42/horizon-mod-schematics/releases/latest |
| replay_recorder | v2.0.0 | https://github.com/lilyco-42/horizon-mod-replay | https://github.com/lilyco-42/horizon-mod-replay/releases/latest |
| skins_3d | v2.0.1 | https://github.com/lilyco-42/horizon-mod-skins-3d | https://github.com/lilyco-42/horizon-mod-skins-3d/releases/latest |

## 快速开始

```bash
cargo test --workspace
./start.sh
```

## 配套客户端（kuake-studio v0.5.19）

夸克网盘客户端已发布全平台安装包（Windows msi/zip、Linux、macOS、Android APK、鸿蒙 OpenHarmony NAPI 原生库）：
https://github.com/lilyco-42/kuake-studio/releases/latest
