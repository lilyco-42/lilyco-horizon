#!/usr/bin/env bash
# lilyco-horizon v3.8.0 发布脚本（2026-11-25 准备，网络恢复后执行）
set -euo pipefail

REPO_DIR="$HOME/项目/lilyco-horizon"
cd "$REPO_DIR"

# 1. 推送 main 与 tag（工作区应干净；tag v3.8.0 已在本地打好）
git push origin main
git push origin v3.8.0

# 2. 创建主仓 Release（源码 tag release，产物在各 mod 子仓）
gh release create v3.8.0 \
  --repo lilyco-42/lilyco-horizon \
  --title "lilyco-horizon v3.8.0" \
  --notes-file RELEASE_NOTES_v3.8.0.md

# 3. 补 kuake-studio 的 About 描述（提升 GitHub 搜索命中）
gh repo edit lilyco-42/kuake-studio \
  --description "夸克网盘客户端：Windows/Linux/macOS 安装包 + Android APK + 鸿蒙 OpenHarmony NAPI 原生库，Rust 原生 API"

# 4. 验收输出
echo '=== 验收 ==='
gh release view v3.8.0 --repo lilyco-42/lilyco-horizon --json tagName,url --jq '"release: " + .tagName + " " + .url'
gh repo view lilyco-42/kuake-studio --json description --jq '"desc: " + .description'
echo '粉丝直达链接:'
echo '  https://github.com/lilyco-42/lilyco-horizon/releases/latest'
echo '  https://github.com/lilyco-42/kuake-studio/releases/latest'
