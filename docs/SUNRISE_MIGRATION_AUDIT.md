# Sunrise 迁移可行性审计（2026-11-28）

## 结论先行

**Sunrise 是我们原上游 Horizon 的改名续作，不是更优的分叉**——`horizon-rust/horizon` 已 404，`sunrise-org/horizon` 重定向到 `sunrise-org/sunrise`，同一批人（Ludovic、arismoko）迁组织改名后继续推进。

| 迁移对象 | 判定 | 工作量 |
|---|---|---|
| ysm_models → Sunrise mod API | **可迁移**（非机械改名，hook 签名层重写） | 8–15 人日，约 600–800 行改写 |
| ohos/android 客户端 → Sunrise 基线 | **不建议 rebase**（结构级重写，≈重做一遍移植） | 数人周，且需持续追高频变更 |
| 总体建议 | **把 Sunrise 当参考源 + 选择性补丁源，不切换上游** | — |

我们真正的护城河（移动端 + ysm 移植）恰好是 Sunrise 的**完全空白区**。

## 1. Sunrise 现状（确认事实）

- 865MB、555 分支、1592 open issues、2740 merged PR，2026-11-28 仍在推送
- main 最近 `293a27f` 单笔改 19963 行（渲染 frame graph / custom lightmap / mod API rollup）
- 自测性能：16 chunks 2074 FPS vs 原版 1661；启动 2.9s vs 7.5s；833 项 parity 差异 0 open
- crate 全量改名 `horizon-* → sunrise-*`，新增 21 个桌面/服务端 crate，删除 15 个（含 `horizon-ohos`）
- 许可证：LGPL-3.0（我们基线）→ PolyForm Shield + Additional Terms（Sunrise）；其 `LICENSING.md` 明确允许 free/open 的移动端 fork，但两套许可不兼容，混合需隔离审查

## 2. ysm_models 迁移审计

### 判定：可迁移（中高工作量）

### 核心利好：3D 渲染内核几乎原样可用
`draw.quad/cube/item`、`CubeFace::new`、`ItemDisplayContext`、`MobRender`、`PlayerRender` 在 Sunrise 全部存在且签名一致/高度兼容：
- `ytree_engine.rs`（1779 行）：仅 ~10 行 API 接触点，签名全兼容 → 基本不动
- baked model / `draw_ysm_real_model`（335 行）：签名一致 → 基本不动
- `hotload.rs`（165 行）：纯 std/fs/png → 完全不动

### 工作量集中区（约 600–800 行重写）
1. **hook 签名层**：Sunrise 全面改为 generator 声明式签名（无 host 参数、out 参数、无 Flow），ysm 全部 20 个 hook 都要改。`Flow::Cancel` → `*canceled = true`
2. **`camera_shake` 完全缺失**（唯一真正 0 命中且无替代）→ 功能永久丢失
3. **Settings 字符串键 → owner-bound `SettingKey`**：7+ 处 `setting_choice("persona")` 要改为持有 `SettingKey` 静态变量
4. **`mod_image` 单槽贴图 → `create_dynamic_texture` + `render_type` 体系**：热加载模型贴图路径重写；`textured_quad` 在 `render_arm`/`render_hand` 不可用，影响第一人称手臂
5. **`consume_click` 缺失**：Y/G/O 按键消费改为 `input_key` 自行边沿检测
6. **GUI**（`gui.rs` 288 行）：~150–200 行重写（`Corner` 布局体系不同）
7. **manifest**：`horizon-mod.toml` → `sunrise-mod.toml`（`[native]` 下 `horizon=` → `sunrise=`，重新 pin SHA）；宏 `#[horizon_mod]` → `#[sunrise_mod]`；构建命令改 `cargo sunrise-mod check/build`

### 修正两个此前的误判
- `render_player_pre` **并非缺失**：Sunrise 有 `hooks/render_player_pre.rs`（v6），签名 `(player, who, draw, model: FrameModelEdit, canceled)`，比 ysm 更强；注意 v5→v6 从 `event.hidden` 迁到 `event.model` 的破坏变更
- `get_arm_pose` **存在**：签名是 out 参数 + `HumanoidRenderState`；ysm 当前实际 `return None`（仅采集状态），语义可保留，重写数据提取路径即可

### 官方参考
- `sunrise-org/sunrise-machine-sample`（标准写法）、`sunrise-minimap`/`sunrise-replay`（完整 native mod 源码）
- `docs/modding/src/how-to/client/{model-drawing,entity-layers,entity-renderers,special-item-renderers,gui-paint}.md`
- `sunrise-org/sunrise-mods` 只是注册表，无可借鉴实现代码；无 YSM/3D 角色类现成 mod

## 3. ohos/android 客户端迁移审计

### 判定：不建议 rebase（结构级重写）

### 漂移量（确认事实）
- `compare c98a51a...main`：`ahead_by = 40846`（API 封顶 10000，20 天跨度），远超可 rebase 范围
- 本地基线 `b45d73d` 在 sunrise 仓库不存在（独立 fork 历史）→ **只能 patch 移植，不能 git rebase**
- 结构级冲突：`crates/sunrise-client/src/game.rs` 与 `src/game/` 均 404，改为扁平 `game_facade.rs` re-export + `sunrise-client-gameplay` 等新 crate；我们的 `window_handler.rs` OHOS 分支、`touch_ui.rs`（8899 行）、`touch*.rs`（~7000 行）失去挂载点

### 移动端支撑缺口
- **winit 不可直接套用**：Sunrise 治理模式从「远程 fork patch」改为「vendored third_party/winit」（针对 X11 修复的 0.30.13 快照），与 richerfu 的 OHOS fork 是两个基础；支持 OHOS 需跨 fork rebase（中高）或重写窗口层（高）
- **wgpu-hal 可复用**：三家共用同版本 30.0.1 vendored patch，可单独同步
- **`.cargo/config.toml` 可无冲突合并**：Sunrise 只占 `[alias] xtask`，OHOS target 段可直接追加（注意去掉绝对路径）

### 若强行 rebase 的工作量（≈重做移动端移植）
61 个 crate 改名（机械低）→ 新建 `sunrise-ohos`（中）→ touch_ui/touch 渲染（高）→ 事件循环/window_handler 重写（高）→ 渲染 gfx/*（高）→ settings（中高）→ mod API（高）→ winit fork 重做（高）。

### 建议的借鉴策略
- 逐项 diff sunrise 的具体 bugfix（wgpu-hal #10124/#10714、naga GLSL、libmimalloc POPCNT）看本地 vendored 是否已含
- 只同步同版本的 `third_party/wgpu-hal` 等目录（风险低）
- 继续沿 `horizon-rust/horizon` main 维护 `ohos-port`，不跨 fork

## 4. 最终建议

1. **不切换上游**。移动端工作是 Sunrise 空白区，切换＝放弃护城河 + 重做移植。
2. **Sunrise 作为跟踪参考**：可在伞形 monorepo 加一个只读 reference（记录 commit/关键改进），不设为 submodule 依赖。
3. **ysm 迁移单独评估启动**：若需要 Sunrise 生态（Modrinth/.smod/Java modpack 数据导入），按第 2 节做 8–15 人日迁移；否则维持现有 Horizon/Starling 线。
4. **许可证**：任何双向代码合并前做正式许可矩阵审查（LGPL vs PolyForm Shield），尤其 `touch_ui.rs` 这类新增文件的版权归属。

## 证据命令
```bash
gh api repos/horizon-rust/horizon                       # 404
gh api repos/sunrise-org/horizon --jq '.full_name'      # → sunrise-org/sunrise
gh api repos/sunrise-org/sunrise/contents/crates --jq '.[].name'
gh api repos/sunrise-org/sunrise/compare/c98a51a...main --jq '{ahead_by,total_commits}'
gh api repos/sunrise-org/sunrise/contents/crates/sunrise-client/src/game.rs  # 404
gh api repos/sunrise-org/sunrise/contents/crates/sunrise-mod-vocab/src/hooks/get_arm_pose.rs -H 'Accept: application/vnd.github.raw+json'
gh api repos/sunrise-org/sunrise/contents/crates/sunrise-mod-vocab/src/host/draw.rs -H 'Accept: application/vnd.github.raw+json'
gh api repos/sunrise-org/sunrise/contents/LICENSING.md -H 'Accept: application/vnd.github.raw'
```
