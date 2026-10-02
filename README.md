# LongFred

Wireless physical throttle client for BigFred (WiThrottle / Z21).

## Hardware variants

Build-time Cargo features (mutually exclusive):

| Feature | Description |
|---------|-------------|
| `variant-longfred-standard` (default) | OLED 128×64, MCP23017×2, 5-way + F-keys + encoder |
| `variant-longfred-mini` | Same as standard, OLED 128×32 |
| `variant-markwtech` | Keypad + 2.42" OLED, WiTcontroller-style (ESP32-C6-DevKitC-1) |
| `variant-markwtech-v1-1` | Same as markwtech; Unexpected Maker TinyC6 pin map |
| `variant-heiko-wifred` | Headless wiFred-style (LEDs + pot), Wi‑Fi config only |

Docs: [ARCHITECTURE.md](ARCHITECTURE.md), [docs/hardware/](docs/hardware/)
([MarkWTech v1.0](docs/hardware/markwtech/1.0.md) / [v1.1 TinyC6](docs/hardware/markwtech/v1.1.md)),
provisioning: [docs/provisioning.md](docs/provisioning.md).

```bash
cargo build -p longfred-firmware --release --bin longfred
cargo build -p longfred-firmware --release --bin longfred \
  --no-default-features --features variant-longfred-mini
make build VARIANT=markwtech
make build VARIANT=markwtech-v1-1
make flash VARIANT=markwtech-v1-1 BATTERY_FACTOR=1.72
```

`BATTERY_FACTOR` (or `LONGFRED_BATTERY_FACTOR` when invoking cargo) overrides the variant's pin-map divider ratio, applied to calibrated pin millivolts (`pin_mv * factor`). Omit it to keep the stock default (`3.76` on TinyC6, `2.0` elsewhere).

## Host tests

```bash
cargo test -p longfred-proto --target x86_64-unknown-linux-gnu
```
