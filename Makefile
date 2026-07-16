# LongFred — common Cargo tasks (run from this directory).
#
#   make build         # ESP32-C6 firmware (riscv32imac)
#   make build-wokwi   # same build + stage ELF for Wokwi simulator
#   make test          # host tests for longfred-proto

.PHONY: all build build-wokwi test help

all: build test

help:
	@echo "Targets:"
	@echo "  build        - cargo build -p longfred-firmware"
	@echo "  build-wokwi  - build + copy ELF to wokwi/longfred"
	@echo "  test         - cargo test -p longfred-proto (host)"
	@echo "  all          - build + test (default)"

build:
	cargo build -p longfred-firmware

build-wokwi:
	./scripts/wokwi-prep.sh

test:
	cargo test -p longfred-proto --target x86_64-unknown-linux-gnu
