#!/usr/bin/env bash
# Sunrise migration helper for mods/ysm_models_sunrise.
# Installs/uses cargo-sunrise-mod, fetches the pinned SDK, and checks the mod.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"          # mods/ysm_models_sunrise
ROOT="$(cd "$HERE/../.." && pwd)"                 # umbrella root
export PATH="$HOME/.local/bin:$PATH"

cmd="${1:-check}"

case "$cmd" in
  tool)
    # Install the Sunrise mod tool per the official signed Linux x86_64 flow.
    if command -v cargo-sunrise-mod >/dev/null; then
      cargo sunrise-mod --version
      exit 0
    fi
    (
      set -eu
      BASE=https://github.com/sunrise-org/sunrise/releases/download
      KEY='r53xb3cuafGxZSicLz06KC+xXp9NJdPH8snNmXo5pAc='
      OPENSSL=openssl
      TOOL=tool-x86_64-unknown-linux-gnu.tar.xz
      COMMIT=$(curl -fsSL "$BASE/nightly/sdk.json" | sed -n 's/.*"commit": *"\([0-9a-f]\{40\}\)".*/\1/p' | head -n 1)
      [ -n "$COMMIT" ] || { echo "could not read the Nightly's build" >&2; exit 1; }
      RELEASE=sdk-$COMMIT
      cd "$(mktemp -d)"
      curl -fsSLO "$BASE/$RELEASE/$TOOL"
      curl -fsSLO "$BASE/$RELEASE/SHA256SUMS"
      curl -fsSLO "$BASE/$RELEASE/SHA256SUMS.sig"
      printf 'horizon release %s\n' "$RELEASE" | cat - SHA256SUMS > message
      { printf '\060\052\060\005\006\003\053\145\160\003\041\000'; printf '%s' "$KEY" | "$OPENSSL" base64 -d -A; } > key.der
      signed=
      while read -r line || [ -n "$line" ]; do
        printf '%s' "$line" | "$OPENSSL" base64 -d -A > signature
        "$OPENSSL" pkeyutl -verify -rawin -pubin -keyform DER -inkey key.der -in message -sigfile signature >/dev/null 2>&1 && signed=1
      done < SHA256SUMS.sig
      [ -n "$signed" ] || { echo "SHA256SUMS is not signed by Sunrise" >&2; exit 1; }
      grep " $TOOL\$" SHA256SUMS | sha256sum -c -
      tar -xJf "$TOOL"
      install -Dm755 cargo-sunrise-mod "$HOME/.local/bin/cargo-sunrise-mod"
      echo "installed cargo-sunrise-mod ($COMMIT)"
    )
    ;;
  check|build|test|pin)
    command -v cargo-sunrise-mod >/dev/null || { echo "run '$0 tool' first" >&2; exit 1; }
    cd "$HERE"
    cargo sunrise-mod "$cmd" "${@:2}"
    ;;
  sync-src)
    # Re-copy the Horizon-side ysm_models sources into this Sunrise project, then
    # re-apply the known mechanical patches (Cargo dep, macro name). Use after editing
    # the Horizon mod to keep both trees aligned.
    SRC="$ROOT/mods/ysm_models"
    [ -d "$SRC/src" ] || { echo "no mods/ysm_models/src" >&2; exit 1; }
    cp "$SRC"/src/*.rs "$HERE/src/"
    sed -i 's#horizon-mod-api = { path = ".horizon/sdk/api/crates/horizon-mod-api" }#sunrise-mod-api = { path = ".sunrise/sdk/api/crates/sunrise-mod-api" }#' "$HERE/Cargo.toml" || true
    sed -i 's/mod_api::horizon_mod/mod_api::sunrise_mod/' "$HERE/src/lib.rs"
    sed -i 's#const MOD_IMAGE_SPRITE: &str = "horizon/modimage";#const MOD_IMAGE_SPRITE: \&str = "ysm_models:modimage";#' "$HERE/src/hotload.rs"
    echo "synced sources; run '$0 check'"
    ;;
  *)
    echo "用法: $0 {tool|check|build|test|pin|sync-src}" >&2
    exit 2
    ;;
esac
