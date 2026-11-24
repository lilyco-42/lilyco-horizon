#!/usr/bin/env bash
set -euo pipefail
ROOT="/home/lilyco/项目/lilyco-horizon"

echo "[1/2] Starting lilyco-horizon-daemon on http://127.0.0.1:28787 ..."
(cd "$ROOT" && cargo run --quiet --bin lilyco-horizon-daemon -- --config "$ROOT/config/studio.toml") &
DAEMON_PID=$!

trap 'kill "$DAEMON_PID" 2>/dev/null || true' EXIT

echo "[2/2] Starting Web Studio GUI on http://127.0.0.1:5178 ..."
cd "$ROOT/apps/web-gui"
pnpm dev
