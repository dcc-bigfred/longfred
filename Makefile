# LongFred — common Cargo tasks (run from this directory).
#
#   make build                         # default variant (longfred-standard)
#   make build VARIANT=longfred-mini   # single hardware variant
#   make build-all                     # all variants (debug)
#   make build-all-release             # all variants (release)
#   make size                          # flash/RAM report for all variants
#   make test                          # host tests for longfred-proto

CARGO ?= cargo
PACKAGE := longfred-firmware
BIN := longfred

# Hardware variants (Cargo feature = variant-<name>).
VARIANTS := longfred-standard longfred-mini markwtech markwtech-v1-1 heiko-wifred
VARIANT ?= longfred-standard

ifeq ($(filter $(VARIANT),$(VARIANTS)),)
$(error unknown VARIANT='$(VARIANT)'; choose one of: $(VARIANTS))
endif

# MarkWTech v1.0 cannot nest print-auto in the Cargo feature (v1.1 would then
# enable both esp-println backends). v1.1 pulls print-jtag via its feature.
ifeq ($(VARIANT),markwtech)
FEATURES := --no-default-features --features variant-markwtech,print-auto
else
FEATURES := --no-default-features --features variant-$(VARIANT)
endif
# Isolate Cargo artifacts per variant so feature switches cannot reuse a stale ELF.
TARGET_DIR := target/$(VARIANT)

# TinyC6 native USB Serial/JTAG is usually ttyACM*; DevKitC-1 UART bridge is ttyUSB*.
ifeq ($(VARIANT),markwtech-v1-1)
ESPFLASH_PORT ?= /dev/ttyACM0
else
ESPFLASH_PORT ?= /dev/ttyUSB0
endif

# Optional ADC calibration override (full-cell `suggested_factor` from UART).
#   make flash VARIANT=markwtech-v1-1 BATTERY_FACTOR=3.81
ifdef BATTERY_FACTOR
export LONGFRED_BATTERY_FACTOR := $(BATTERY_FACTOR)
endif

.PHONY: all build build-release build-all build-all-release \
	build-longfred-standard build-longfred-mini build-markwtech \
	build-markwtech-v1-1 build-heiko-wifred \
	flash flash-markwtech flash-markwtech-v1-1 \
	size check-size check-size-only test lint help

all: build test

help:
	@echo "Targets:"
	@echo "  build [VARIANT=...]     - cargo build -p $(PACKAGE) (debug)"
	@echo "  build-release [VARIANT] - release build for one variant"
	@echo "  build-all               - debug build for every variant"
	@echo "  build-all-release       - release build for every variant"
	@echo "  build-<variant>         - shorthand debug builds:"
	@echo "                            $(VARIANTS)"
	@echo "  size / check-size       - release-build all variants + ESP32-C6 flash/RAM report"
	@echo "  check-size-only         - check existing dist/*.elf (no cargo; used by CI)"
	@echo "  test                    - cargo test -p longfred-proto and longfred-ui (host)"
	@echo "  lint                    - rustfmt --check on the workspace"
	@echo "  flash [VARIANT=...]     - cargo run (build + espflash) for VARIANT"
	@echo "  flash-markwtech         - flash v1.0 (DevKitC-1, /dev/ttyUSB0)"
	@echo "  flash-markwtech-v1-1    - flash v1.1 (TinyC6 USB Serial/JTAG, /dev/ttyACM0)"
	@echo "  all                     - build + test (default)"
	@echo ""
	@echo "VARIANT (default: $(VARIANT)): $(VARIANTS)"
	@echo "ESPFLASH_PORT             - override serial device (defaults: ttyUSB0, TinyC6 ttyACM0)"
	@echo "BATTERY_FACTOR            - optional ADC scale override (any variant)"

build:
	$(CARGO) build -p $(PACKAGE) --target-dir $(TARGET_DIR) $(FEATURES)

build-release:
	$(CARGO) build -p $(PACKAGE) --release --bin $(BIN) --target-dir $(TARGET_DIR) $(FEATURES)

build-all:
	@for v in $(VARIANTS); do \
		echo "==> build VARIANT=$$v"; \
		$(MAKE) --no-print-directory build VARIANT=$$v || exit 1; \
	done

build-all-release:
	@for v in $(VARIANTS); do \
		echo "==> build-release VARIANT=$$v"; \
		$(MAKE) --no-print-directory build-release VARIANT=$$v || exit 1; \
	done

build-longfred-standard:
	@$(MAKE) --no-print-directory build VARIANT=longfred-standard

build-longfred-mini:
	@$(MAKE) --no-print-directory build VARIANT=longfred-mini

build-markwtech:
	@$(MAKE) --no-print-directory build VARIANT=markwtech

build-markwtech-v1-1:
	@$(MAKE) --no-print-directory build VARIANT=markwtech-v1-1

build-heiko-wifred:
	@$(MAKE) --no-print-directory build VARIANT=heiko-wifred

# Release-build every variant and verify it fits ESP32-C6 flash partition + on-chip RAM.
size check-size:
	./scripts/check-esp32c6-size.sh

# Verify prebuilt dist/longfred-<variant>-esp32c6.elf files (CI after artifact download).
check-size-only:
	./scripts/check-esp32c6-size.sh --check-only

test:
	$(CARGO) test -p longfred-proto --target x86_64-unknown-linux-gnu
	$(CARGO) test -p longfred-ui --target x86_64-unknown-linux-gnu
	$(CARGO) test -p longfred-ui --target x86_64-unknown-linux-gnu --profile release-assertions

lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy -p longfred-ui --all-targets --target x86_64-unknown-linux-gnu -- --no-deps -D warnings

flash:
	ESPFLASH_PORT=$(ESPFLASH_PORT) $(CARGO) run -p $(PACKAGE) \
	  --target-dir $(TARGET_DIR) $(FEATURES)

flash-markwtech:
	@$(MAKE) --no-print-directory flash VARIANT=markwtech

flash-markwtech-v1-1:
	@$(MAKE) --no-print-directory flash VARIANT=markwtech-v1-1
