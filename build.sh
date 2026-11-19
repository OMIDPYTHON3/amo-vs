#!/usr/bin/env bash
# AMO VS cross-platform build script
set -euo pipefail

cd "$(dirname "$0")"

DIST="dist"
BIN="amo-vs"
PROFILE="${1:-release}"

case "$PROFILE" in
  debug|dev) CARGO_ARGS=() ; PROFILE_DIR="debug" ;;
  release)   CARGO_ARGS=(--release) ; PROFILE_DIR="release" ;;
  all)
    echo "==> Cross-compiling all supported targets"
    TARGETS=(
      x86_64-unknown-linux-gnu
      aarch64-unknown-linux-gnu
      aarch64-linux-android
      x86_64-pc-windows-gnu
      x86_64-apple-darwin
      aarch64-apple-darwin
    )
    mkdir -p "$DIST"
    for target in "${TARGETS[@]}"; do
      echo "--> $target"
      if rustup target list --installed | grep -qx "$target"; then
        cargo build --release --target "$target"
        ext=""
        [[ "$target" == *windows* ]] && ext=".exe"
        cp "target/$target/release/$BIN$ext" "$DIST/$BIN-$target$ext" 2>/dev/null || true
      else
        echo "    skipping (run: rustup target add $target)"
      fi
    done
    exit 0
    ;;
  *)
    echo "Usage: $0 [debug|release|all]" >&2
    exit 2
    ;;
esac

echo "==> Building AMO VS ($PROFILE)"
cargo build "${CARGO_ARGS[@]}"

mkdir -p "$DIST"
cp "target/$PROFILE_DIR/$BIN" "$DIST/$BIN"
chmod +x "$DIST/$BIN"
echo "==> Built $DIST/$BIN"
