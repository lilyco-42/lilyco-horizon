#!/usr/bin/env bash
# Umbrella build entry for the lilyco-horizon family:
#   local tools + native mods + OpenHarmony client + Android/Starling client.
# Nothing here builds all targets at once by default; pick a subcommand.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OHOS="$ROOT/clients/horizon-ohos"
ANDROID="$ROOT/clients/horizon-android"
MODS_DIR="$ROOT/mods"

need() { command -v "$1" >/dev/null || { echo "缺少命令: $1" >&2; return 1; }; }

submodules() {
  echo "[submodules] 拉取全部客户端与 mod 源码"
  git -C "$ROOT" submodule update --init --recursive
  echo "[submodules] 状态:"
  git -C "$ROOT" submodule status
}

tools() {
  echo "[tools] 构建本地工具 workspace"
  cargo build --manifest-path "$ROOT/Cargo.toml" --workspace
  echo "[tools] 测试 ysm-native-port"
  cargo test --manifest-path "$ROOT/Cargo.toml" -p ysm-native-port
}

mods() {
  echo "[mods] 校验各 native mod 仓库"
  for d in "$MODS_DIR"/*; do
    [ -f "$d/Cargo.toml" ] || continue
    echo "-- $(basename "$d")"
    (cd "$d" && grep -m1 '^version' horizon-mod.toml 2>/dev/null || true)
  done
  echo "[mods] 桌面 mod 用 horizon-launcher native-build <dir> --trust-native-code 构建"
  echo "[mods] 安卓 mod 用 clients/horizon-android 的 tools/android_native_sdk.py mods 编入 APK"
}

ohos() {
  need cargo
  [ -f "$OHOS/.cargo/config.toml" ] || { echo "OHOS 工具链配置缺失" >&2; return 1; }
  echo "[ohos] 客户端源码: $OHOS"
  echo "[ohos] 检查 OHOS crate:"
  cargo check --manifest-path "$OHOS/Cargo.toml" -p horizon-ohos 2>/dev/null || \
    echo "(需 OHOS SDK sysroot；见 clients/horizon-ohos/.cargo/config.toml)"
  echo "[ohos] HAP 工程位于 ~/probe/gpui-template/ohos，产物 libhorizon_ohos.so"
}

android() {
  need cargo
  echo "[android] 客户端源码: $ANDROID"
  echo "[android] 用 Starling 工作流构建 APK:"
  echo "  1) 在 $ANDROID 运行 tools/android_native_sdk.py prepare"
  echo "  2) tools/android_native_sdk.py mods OUT mods/*.hmod"
  echo "  3) RUSTFLAGS='-C prefer-dynamic' python3 packaging/android/build_apk.py --abi arm64-v8a"
  echo "[android] 依赖 NDK 28.2.13676358 + cargo-ndk 3.5.4 + aarch64-linux-android"
}

status() {
  echo "=== lilyco-horizon 伞形仓库状态 ==="
  git -C "$ROOT" submodule status
  echo "--- 本地工具 ---"
  grep -E '^members|^    "' "$ROOT/Cargo.toml" | head -10
}

case "${1:-status}" in
  submodules) submodules ;;
  tools)      tools ;;
  mods)       mods ;;
  ohos)       ohos ;;
  android)    android ;;
  status)     status ;;
  *)
    echo "用法: $0 {submodules|tools|mods|ohos|android|status}" >&2
    exit 2
    ;;
esac
