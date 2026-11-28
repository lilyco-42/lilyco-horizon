# ysm_models → Sunrise 迁移进度

目标：把桌面版 `ysm_models`（Horizon `horizon-mod-api`）逐步迁到 `sunrise-org/sunrise` 的 `sunrise-mod-api`，每步可编译、可回退。基线 tag：`v3.9.0-pre-sunrise`。

## 状态：里程碑 1 —— 编译通过

完整 ysm 源码（`lib.rs` 1815 行 + `ytree_engine.rs` 1779 行 + `gui.rs` 288 行 + `hotload.rs` 165 行）在 Sunrise SDK（nightly `94256510`，Rust 1.99.1）下 **`cargo sunrise-mod check` 通过**：

```
ysm models uses 517 items of the API (2 Part kinds, 1 associated item, 152 fields or variants,
6 functions, 22 hooks, 40 hook arguments, 1 macro, 153 methods, 25 traits, 115 types). Made for 9425651.
```

仅 4 个 dead_code 警告，无错误。

### 关键发现（推翻审计的悲观预估）

实际迁移只改了 **3 处机械改动**，不是审计预估的 "8–15 人日重写"：

| # | 改动 | 位置 |
|---|---|---|
| 1 | 依赖 `horizon-mod-api` → `sunrise-mod-api`（路径 `.sunrise/sdk/...`） | `Cargo.toml` |
| 2 | 宏 `#[mod_api::horizon_mod]` → `#[mod_api::sunrise_mod]` | `src/lib.rs:831` |
| 3 | `host.mod_image(w,h,rgba)` → `host.create_dynamic_texture(&Identifier::parse(name), w,h,rgba)`；sprite 名改 `ysm_models:modimage`（mod 命名空间） | `src/lib.rs:1266`、`src/hotload.rs:7` |

**为什么审计高估**：审计基于 sunrise **main**（11-28，hook 已改声明式签名、`game.rs` 重组）。但 `cargo sunrise-mod` 实际 pin 的是 **nightly SDK**，其 `Hooks` 签名仍是我们熟悉的 `fn hook(&self, host, &mut Event)` 风格，`BlockEntityDrawContext` / `quad` / `cube` / `item`、`MobRender`、`GetArmPoseEvent`、`RenderPlayerPreEvent` 全部同名同签名。**应以编译器为准，不要用 audit 的预估。**

## 复现

```bash
mods/ysm_models_sunrise/tools/migrate.sh tool    # 一次性：官方签名安装 cargo-sunrise-mod
mods/ysm_models_sunrise/tools/migrate.sh check   # 拉 SDK 并编译
```

`.sunrise/sdk/` 是工具拉取的签名 SDK，已在 `.gitignore` 排除，勿提交。

## 下一步（待办）

1. **资源 sprite 对齐**：热加载用的 `ysm_models:modimage` 是 GUI 动态贴图，但 3D 模型渲染走世界 `draw.quad(model.sprite,...)`，需要 sprite 出现在 atlas。热加载模型需改为把贴图作为 boke 的静态/动态 sprite 注册策略，验证第一人称/第三人称贴图正确。
2. **实际进游戏验证**：`cargo sunrise-mod build` + 进游戏确认 3D 角色渲染、手势、持物、弓箭姿势（复用桌面版已修的 bug）。
3. **GUI 面板**（`gui.rs`）：`corner`/`DrawContext` 布局体系核对。
4. **打包发布**：`.smod` 产物 + `sunrise-mod.toml` 版本号（当前 0.1.0，随迁移稳定后升到 4.x 对齐）。
5. **回对齐 Horizon 侧**：`tools/migrate.sh sync-src` 从 `mods/ysm_models` 同步源码并自动套用 3 处补丁。

## 回退

`git checkout v3.9.0-pre-sunrise` 可回到迁移前的伞形仓库状态（含原 Horizon mod 线与审计文档）。
