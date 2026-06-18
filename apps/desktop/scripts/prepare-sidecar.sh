#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
BIN_DIR="$ROOT/apps/desktop/src-tauri/binaries"
mkdir -p "$BIN_DIR"
HOST="$(rustc -vV | sed -n 's/host: //p')"

if [ "${UNIVERSAL:-0}" = "1" ]; then
  cargo build --release -p servicio-daemon --target aarch64-apple-darwin --manifest-path "$ROOT/Cargo.toml"
  cargo build --release -p servicio-daemon --target x86_64-apple-darwin  --manifest-path "$ROOT/Cargo.toml"
  lipo -create \
    "$ROOT/target/aarch64-apple-darwin/release/servicio-daemon" \
    "$ROOT/target/x86_64-apple-darwin/release/servicio-daemon" \
    -output "$BIN_DIR/servicio-daemon-universal-apple-darwin"
  echo "staged universal $BIN_DIR/servicio-daemon-universal-apple-darwin"
else
  cargo build --release -p servicio-daemon --manifest-path "$ROOT/Cargo.toml"
  cp "$ROOT/target/release/servicio-daemon" "$BIN_DIR/servicio-daemon-$HOST"
  echo "staged $BIN_DIR/servicio-daemon-$HOST"
fi
