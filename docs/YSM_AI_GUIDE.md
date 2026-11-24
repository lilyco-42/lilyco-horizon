# YSM 模型 AI 制作与操作指南 (`lilyco-horizon`)

## 1. 黄金美学与工程规范
- **不超过 3 个主色彩（60-30-10 法则）**：
  - **60% 主色**：大面积统一基调（如 LYCO 曜石黑 `#16151D ~ #2B2936` 用于头发、外套、裙身、靴子）。
  - **30% 辅色**：干净对比层（如象牙白 `#F2EFEA` 衬衫/袖口/领边 + 肌肤 `#F3DCD1`），严禁把干净白块涂脏。
  - **10% 点缀色**：仅用于 1–2 个视觉焦点（如绯红 `#B82530` 发带与领结），严禁全身零碎乱点色。
- **优先继承官方 CC0 骨架**：
  - 直接从 `ysm-2.6.5` 内置的 `assets/yes_steve_model/builtin/default/`（**CC0 公共领域协议**）派生，完整保留 64 骨标准层级、眨眼/眉毛骨骼与 14 套官方动画（2.8 MB），避免手搓方脸或骨骼撕裂。

## 2. 一键重制脚本
```bash
python3 /home/lilyco/项目/lilyco-horizon/tools/build_lyco_from_cc0_default.py
```
- 自动输出到 Java 版目录：`~/.local/share/PrismLauncher/instances/MC-AI-YSM-Litematica-1.21.1/.minecraft/config/yes_steve_model/custom/lyco_ip.zip`
- 自动备份到桌面：`/home/lilyco/桌面/LYCO_YSM模型/lyco_ip_cc0_v2.zip`

## 3. 游戏内热重载与快捷键（Java 版官方 YSM）
- **热重载模型（无需重启游戏）**：聊天栏输入 `/ysm model reload`
- **切换模型界面**：按 `Alt + Y`
- **动作轮盘**：按 `Alt + Z`（或设置中绑定的动作键）

## 4. 常见加载报错排查
| 报错信息 | 根因 | 解决方式 |
| :--- | :--- | :--- |
| `无法解析模型文件` | 目录下混入了 2 KB 左右的空占位/损坏 `.ysm` 文件 | 将异常小文件移出 `config/yes_steve_model/custom/` |
| `重复的模型` | 两个 `.ysm` 内部声明了相同的模型 ID（如 `DeepSeek酱.ysm` 与 `DeepSeek_DeepSeek酱.ysm`） | 仅保留其中一份 |
