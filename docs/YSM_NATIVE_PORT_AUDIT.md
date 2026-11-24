# YesSteveModel-Native → Rust (`ysm-native-port` & Horizon `ysm_models v3.0`) 逐文件移植与测试对照表

| 源 C++ 文件 (`YesSteveModel-Native`, Apache-2.0) | Rust 移植目标 | 核心职责 | 自动化测试与对比验证 |
| :--- | :--- | :--- | :--- |
| `modules/gfx/src/math/euler.h` | [euler.rs](file:///home/lilyco/项目/lilyco-horizon/crates/ysm-native-port/src/math/euler.rs) | 位级 `ModuloTwoPi`、`ReduceAngle`、`EulerZYX` 列主序矩阵 | 1:1 移植 `euler_test.cc`（误差 `< 2e-6`、极大角归约全通过） |
| `modules/gfx/include/gfx/math/pose_stack.h`<br>`modules/gfx/src/math/pose_stack_impl.inc` | [pose_stack.rs](file:///home/lilyco/项目/lilyco-horizon/crates/ysm-native-port/src/math/pose_stack.rs) | 层级 `PoseStack`（`Push/Pop/Translate/RotateZYX/Scale`、`normal_scale`、`tangent_orientation`） | 1:1 移植 `pose_stack_test.cc` 全通过 |
| `modules/gfx/include/gfx/renderer/bone_attribute.h`<br>`modules/gfx/src/renderer/model_state_impl.inc` | [bone_attribute.rs](file:///home/lilyco/项目/lilyco-horizon/crates/ysm-native-port/src/renderer/bone_attribute.rs)<br>[model_state.rs](file:///home/lilyco/项目/lilyco-horizon/crates/ysm-native-port/src/renderer/model_state.rs) | DFS 先序 `ModelState::Extract`、`ApplyLocalState`、`subtree_end` O(1) 跳过 | **肘/腕关节连续性数学证明测试（0.000mm 误差，数学上杜绝“断手”）** + `children_hidden` 子树跳过测试 |
| Bedrock `minecraft:geometry` | [geo.rs](file:///home/lilyco/项目/lilyco-horizon/crates/ysm-native-port/src/codec/geo.rs) | 全量解析 `main.json` 骨骼树、Box/Per-Face UV、镜像与立方体旋转 | 实测无损解析 173 骨 `DeepSeek_DeepSeek酱`（>3000 面）与 64 骨 CC0 `lyco_ip` |
| Bedrock Molang 表达式引擎 | [molang/mod.rs](file:///home/lilyco/项目/lilyco-horizon/crates/ysm-native-port/src/molang/mod.rs) | 角度制 `math.*`、`q.*`/`query.*`、`v.*`、`t.*`、三元与比较运算 | 实测覆盖 `main.animation.json` 表达式零报错 |
| Bedrock `.animation.json` | [animation.rs](file:///home/lilyco/项目/lilyco-horizon/crates/ysm-native-port/src/codec/animation.rs) | 线性 + Catmull-Rom 关键帧采样器与 Molang 通道求值 | 实测解析并求值真实 `DeepSeek_DeepSeek酱/animations/main.animation.json` |
| 二进制 `.ytree` 序列化 & Horizon 集成 | [ytree.rs](file:///home/lilyco/项目/lilyco-horizon/crates/ysm-native-port/src/codec/ytree.rs)<br>[ytree_engine.rs](file:///home/lilyco/.local/share/horizon/horizon-native-mods/ysm_models/src/ytree_engine.rs) | 无损编码 64~557 骨完整骨骼树 + 实时眨眼、发丝惯性、肘膝联动、裙摆随腿外摆、潜行/跳跃 | 9 款模型全部烘焙为精确 `1.82m` `.ytree`，官方 `horizon-launcher native-build` 零警告编入 `ysm_models-3.0.0.hmod` |

---

## Official YSM Item & 3D Action Parity Map (`InnerClassify.java` / `ConditionalHold.java` / `ConditionalUse.java` / `ConditionalSwing.java`)

Verified directly against `YesSteveModel/YesSteveModel` (Apache-2.0) and local PrismLauncher `ysm-2.6.5-fabric+mc1.21.1-release.jar`:

| Item / State | Official YSM Classifier (`InnerClassify.java`) | Official YSM Animation Clip(s) | Horizon `ysm_models` v3.6.0 Behavior |
| --- | --- | --- | --- |
| **Plain Blocks / Misc Items** (`obsidian`, `cobblestone`, `dirt`, `rail`, etc.) | `EMPTY` (`""`) + `UseAnim.NONE` | **None** (`ConditionalHold` returns `""`) | **No forced arm override** — arm stays natural with `idle / walk / run / sneak / ride`; 3D block still renders on `RightHandLocator` |
| **Sword / Katana** | `"sword"` | `hold_mainhand:sword`, `swing:sword` / `sword_attack_01` | Slot 17 (`hold_mainhand:sword`) + Slot 25 (`swing:sword`) + 3D sword in hand |
| **Bow** | `"bow"` | `hold_mainhand:bow`, `use_mainhand:bow` | Slot 18 (`hold_mainhand:bow`) + Slot 26 (`use_mainhand:bow`) + 3D bow |
| **Crossbow** | `"crossbow"` | `hold_mainhand:charged_crossbow` / `hold_mainhand:crossbow` | Slot 19 + 3D crossbow |
| **Tools** (`pickaxe`/`axe`/`shovel`/`hoe`) | `"pickaxe"` / `"axe"` / `"shovel"` / `"hoe"` | `hold_mainhand:pickaxe` / `axe` / `shovel` / `hoe` | Slot 20 + 3D tool |
| **Shield** | `"shield"` | `hold_mainhand:shield` / `hold_offhand:shield` | Slot 21 + 3D shield |
| **Fishing Rod** (`minecraft:fishing_rod`) | `"fishing_rod"` | `hold_mainhand:fishing_rod` / `hold_mainhand:fishing`, `swing:fishing_rod` | Slot 22 (`hold_mainhand:fishing_rod` / `fishing`) + right-click cast swing + 3D rod |
| **Trident / Mace** | `"spear"` | `hold_mainhand:spear`, `hold_mainhand$minecraft:mace` | Slot 23 + 3D weapon |
| **Eat / Drink** | `UseAnim.EAT` / `DRINK` | `use_mainhand:eat` / `use_mainhand:drink` | Slot 27 + 3D food/potion |
| **Riding Horse / Boat / Minecart** | `PlayerMainPredicate` (`isPassenger`) | `ride` / `sit` / `boat` / `ride_pig` | Slot 28 (`SLOT_RIDE`) via `p.entity().is_passenger(host)` + active Layer 2.5 held item |
