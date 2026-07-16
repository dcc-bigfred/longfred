#!/usr/bin/env bash
# Build LongFred firmware and stage the ELF for Wokwi (wokwi/longfred).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Ensure we write into this workspace, not a redirected sandbox target dir.
unset CARGO_TARGET_DIR

# Wokwi has no real WiFi radio; build with the `sim` feature (skips net bring-up).
PROFILE="${1:-debug}"
case "$PROFILE" in
  debug)  cargo build -p longfred-firmware --features sim ;;
  release) cargo build -p longfred-firmware --release --features sim ;;
  *)
    echo "usage: $0 [debug|release]" >&2
    exit 1
    ;;
esac

SRC="target/riscv32imac-unknown-none-elf/${PROFILE}/longfred"
DST="wokwi/longfred"

if [[ ! -f "$SRC" ]]; then
  echo "error: missing $SRC" >&2
  exit 1
fi

mkdir -p wokwi
cp -f "$SRC" "$DST"
echo "Staged $SRC -> $DST ($(du -h "$DST" | cut -f1))"
echo "Start simulator: F1 → Wokwi: Start Simulator"
