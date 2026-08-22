#!/usr/bin/env bash
# Report flash + static RAM usage for each LongFred variant against ESP32-C6 limits.
#
# Flash budget: dual-slot OTA app partition in partitions.csv (ota_0 / ota_1 = 0x3C0000).
# RAM budget:   esp-hal esp32c6 memory.x RAM LENGTH (0x6E610) — linker already enforces
#               this; we re-check sections and fail if anything looks over.
#
# Usage:
#   ./scripts/check-esp32c6-size.sh              # build each variant, then check
#   ./scripts/check-esp32c6-size.sh --check-only # check existing dist/*.elf (no cargo)
#   VARIANTS="markwtech heiko-wifred" ./scripts/check-esp32c6-size.sh --check-only
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

TARGET="${TARGET:-riscv32imac-unknown-none-elf}"
CHIP="${CHIP:-esp32c6}"
BIN="${BIN:-longfred}"
DIST_DIR="${DIST_DIR:-dist}"
# From esp-hal ld/esp32c6/memory.x: RAM LENGTH = 0x6E610
RAM_LIMIT_BYTES="${RAM_LIMIT_BYTES:-$((0x6E610))}"
OTA_SLOT_BYTES="${OTA_SLOT_BYTES:-$((0x3C0000))}"
PARTITION_TABLE="${PARTITION_TABLE:-$ROOT/partitions.csv}"
VARIANTS=(${VARIANTS:-longfred-standard longfred-mini markwtech markwtech-v1-1 heiko-wifred})

CHECK_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --check-only) CHECK_ONLY=1 ;;
    -h|--help)
      sed -n '2,12p' "$0"
      exit 0
      ;;
    *)
      echo "error: unknown argument: $arg" >&2
      exit 1
      ;;
  esac
done

if ! command -v espflash >/dev/null 2>&1; then
  echo "error: espflash not found in PATH" >&2
  exit 1
fi
if ! command -v readelf >/dev/null 2>&1; then
  echo "error: readelf not found in PATH" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

# On-chip RAM section usage (0x4080_0000 .. 0x4088_0000).
# Prints: static_bytes stack_bytes total_bytes
# Stack is sized by the linker to fill leftover RAM, so total ≈ RAM_LIMIT after a
# successful link; static (rwtext+data+bss+…) is the meaningful footprint.
ram_breakdown_bytes() {
  local elf="$1"
  readelf -SW "$elf" | awk '
    /^\s*\[[ 0-9]+\]/ {
      line = $0
      sub(/^\s*\[[ 0-9]+\]\s+/, "", line)
      n = split(line, a, /[[:space:]]+/)
      if (n < 5) next
      name = a[1]
      addr = strtonum("0x" a[3])
      size = strtonum("0x" a[5])
      if (addr < 0x40800000 || addr >= 0x40880000) next
      if (name == ".stack") stack += size
      else static += size
    }
    END { printf "%d %d %d", static + 0, stack + 0, static + stack + 0 }
  '
}

human() {
  local n="${1:-0}"
  if (( n >= 1048576 )); then
    awk -v n="$n" 'BEGIN { printf "%.2f MiB", n/1048576 }'
  elif (( n >= 1024 )); then
    awk -v n="$n" 'BEGIN { printf "%.1f KiB", n/1024 }'
  else
    printf "%d B" "$n"
  fi
}

elf_path_for_variant() {
  local variant="$1"
  printf '%s/%s-%s-esp32c6.elf' "$DIST_DIR" "$BIN" "$variant"
}

printf "%-18s %12s %12s %8s  %12s %12s %12s %8s  %s\n" \
  "VARIANT" "FLASH_USED" "FLASH_MAX" "FLASH%" "RAM_STATIC" "RAM_STACK" "RAM_MAX" "STATIC%" "STATUS"
printf "%-18s %12s %12s %8s  %12s %12s %12s %8s  %s\n" \
  "------------------" "------------" "------------" "--------" \
  "------------" "------------" "------------" "--------" "------"

failed=0

for variant in "${VARIANTS[@]}"; do
  elf="$(elf_path_for_variant "$variant")"

  if (( CHECK_ONLY )); then
    if [[ ! -f "$elf" ]]; then
      echo "error: missing ELF for ${variant}: ${elf}" >&2
      failed=1
      continue
    fi
  else
    # Isolate artifacts per variant so feature switches cannot reuse a stale ELF.
    # Same layout as Makefile TARGET_DIR=target/$(VARIANT).
    target_dir="target/${variant}"
    built_elf="${target_dir}/${TARGET}/release/${BIN}"

    echo "==> release build variant-${variant}" >&2
    features="variant-${variant}"
    if [[ "$variant" == "markwtech" ]]; then
      features="variant-markwtech,print-auto"
    fi
    if ! cargo build -p longfred-firmware --release --bin "$BIN" \
        --target-dir "$target_dir" \
        --no-default-features --features "$features" \
        >"${tmpdir}/${variant}.cargo.log" 2>&1; then
      echo "error: cargo build failed for variant-${variant}" >&2
      tail -n 40 "${tmpdir}/${variant}.cargo.log" >&2
      failed=1
      continue
    fi

    if [[ ! -f "$built_elf" ]]; then
      echo "error: missing ELF for ${variant}: ${built_elf}" >&2
      failed=1
      continue
    fi

    mkdir -p "$DIST_DIR"
    cp -f "$built_elf" "$elf"
  fi

  img="${tmpdir}/${variant}.bin"
  log="${tmpdir}/${variant}.espflash.log"
  if ! ESPFLASH_SKIP_UPDATE_CHECK=true espflash save-image \
      --chip "$CHIP" --partition-table "$PARTITION_TABLE" --merge "$elf" "$img" >"$log" 2>&1; then
    printf "%-18s %12s %12s %8s  %12s %12s %12s %8s  %s\n" \
      "$variant" "-" "-" "-" "-" "-" "-" "-" "FAIL (espflash)"
    sed -n '1,20p' "$log" >&2
    failed=1
    continue
  fi

  # e.g. "App/part. size:    764,320/4,128,768 bytes, 18.51%"
  flash_line="$(grep -E 'App/part\. size:' "$log" | tail -n1 || true)"
  if [[ -z "$flash_line" ]]; then
    printf "%-18s %12s %12s %8s  %12s %12s %12s %8s  %s\n" \
      "$variant" "-" "-" "-" "-" "-" "-" "-" "FAIL (no size line)"
    failed=1
    continue
  fi

  flash_used="$(sed -E 's/.*App\/part\. size:[[:space:]]*([0-9,]+)\/.*/\1/; s/,//g' <<<"$flash_line")"
  flash_max="$(sed -E 's/.*App\/part\. size:[[:space:]]*[0-9,]+\/([0-9,]+).*/\1/; s/,//g' <<<"$flash_line")"
  flash_pct="$(awk -v u="$flash_used" -v m="$flash_max" 'BEGIN { printf "%.2f", (u*100)/m }')"

  # Prefer a portable capture; `read < <(fn)` + `set -e` aborts on some bash/pipefail combos.
  ram_breakdown="$(ram_breakdown_bytes "$elf")"
  ram_static="${ram_breakdown%% *}"
  rest="${ram_breakdown#* }"
  ram_stack="${rest%% *}"
  ram_total="${rest##* }"
  static_pct="$(awk -v u="$ram_static" -v m="$RAM_LIMIT_BYTES" 'BEGIN { printf "%.2f", (u*100)/m }')"

  status="OK"
  if (( flash_used > flash_max )); then
    status="FAIL (flash)"
    failed=1
  fi
  if (( flash_used > OTA_SLOT_BYTES )); then
    status="FAIL (ota slot)"
    failed=1
  fi
  if (( ram_total > RAM_LIMIT_BYTES )); then
    status="FAIL (ram)"
    failed=1
  fi

  printf "%-18s %12s %12s %7s%%  %12s %12s %12s %7s%%  %s\n" \
    "$variant" \
    "$(human "$flash_used")" "$(human "$flash_max")" "$flash_pct" \
    "$(human "$ram_static")" "$(human "$ram_stack")" "$(human "$RAM_LIMIT_BYTES")" "$static_pct" \
    "$status"
done

echo
echo "Limits: ESP32-C6 OTA app slot 0x3C0000 (partitions.csv) + on-chip RAM 0x6E610 from esp-hal memory.x"
echo "Note: RAM_STATIC includes .bss (with 72 KiB esp_alloc heap); linker fills leftover with .stack."

if (( failed != 0 )); then
  echo "error: one or more variants exceed ESP32-C6 memory budget" >&2
  exit 1
fi

echo "All variants fit in ESP32-C6 flash and RAM."
