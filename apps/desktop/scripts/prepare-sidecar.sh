#!/usr/bin/env bash
# Build the release daemon and stage it as a Tauri externalBin (triple-suffixed).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
TRIPLE="$(rustc -vV | sed -n 's/host: //p')"
BIN_DIR="$ROOT/apps/desktop/src-tauri/binaries"
mkdir -p "$BIN_DIR"
cargo build --release -p servicio-daemon --manifest-path "$ROOT/Cargo.toml"
cp "$ROOT/target/release/servicio-daemon" "$BIN_DIR/servicio-daemon-$TRIPLE"
echo "staged $BIN_DIR/servicio-daemon-$TRIPLE"
