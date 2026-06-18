#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
BIN_DIR="$ROOT/apps/desktop/src-tauri/binaries"
mkdir -p "$BIN_DIR"
HOST="$(rustc -vV | sed -n 's/host: //p')"

if [ "${UNIVERSAL:-0}" = "1" ]; then
  # Tauri's `--target universal-apple-darwin` needs BOTH: a per-arch sidecar for each
  # per-arch compile, AND a lipo'd `-universal-apple-darwin` sidecar for the final bundle
  # copy. Stage all three.
  cargo build --release -p servicio-daemon --target aarch64-apple-darwin --manifest-path "$ROOT/Cargo.toml"
  cargo build --release -p servicio-daemon --target x86_64-apple-darwin  --manifest-path "$ROOT/Cargo.toml"
  cp "$ROOT/target/aarch64-apple-darwin/release/servicio-daemon" "$BIN_DIR/servicio-daemon-aarch64-apple-darwin"
  cp "$ROOT/target/x86_64-apple-darwin/release/servicio-daemon"  "$BIN_DIR/servicio-daemon-x86_64-apple-darwin"
  lipo -create \
    "$ROOT/target/aarch64-apple-darwin/release/servicio-daemon" \
    "$ROOT/target/x86_64-apple-darwin/release/servicio-daemon" \
    -output "$BIN_DIR/servicio-daemon-universal-apple-darwin"
  echo "staged sidecars: servicio-daemon-{aarch64,x86_64,universal}-apple-darwin"
else
  # Windows binaries carry a .exe extension; Tauri's sidecar lookup expects
  # `servicio-daemon-<host-triple>.exe` to match the `binaries/servicio-daemon`
  # externalBin entry. Stage with the right extension on each platform.
  EXE=""
  case "$HOST" in
    *windows*) EXE=".exe" ;;
  esac
  cargo build --release -p servicio-daemon --manifest-path "$ROOT/Cargo.toml"
  cp "$ROOT/target/release/servicio-daemon$EXE" "$BIN_DIR/servicio-daemon-$HOST$EXE"
  echo "staged $BIN_DIR/servicio-daemon-$HOST$EXE"
fi
