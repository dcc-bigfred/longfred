# LongFred

Wireless physical throttle client for BigFred (WiThrottle / Z21).

## Hardware variants

Build-time Cargo features (mutually exclusive):

| Feature | Description |
|---------|-------------|
| `variant-longfred-v1` (default) | Custom PCB (ESP32-C6 QFN-40): OLED 0.91" 128×32, MCP map from hardware 004 |
| `variant-markwtech-v1-1` | Keypad + 2.42" OLED on Unexpected Maker TinyC6 (implies `variant-markwtech`) |

Docs: [ARCHITECTURE.md](ARCHITECTURE.md), [docs/hardware/](docs/hardware/)
([LongFred v1](docs/hardware/longfred-v1.md) / [MarkWTech v1.1 TinyC6](docs/hardware/markwtech/v1.1.md)),
provisioning: [docs/provisioning.md](docs/provisioning.md).

```bash
cargo build -p longfred-firmware --release --bin longfred
make build VARIANT=longfred-v1
make build VARIANT=markwtech-v1-1
make flash VARIANT=markwtech-v1-1 BATTERY_FACTOR=1.72
```

`BATTERY_FACTOR` (or `LONGFRED_BATTERY_FACTOR` when invoking cargo) overrides the variant's pin-map ADC scale (`raw * factor` millivolts). Omit it to keep the stock default (`1.7`).

## Host tests

```bash
cargo test -p longfred-proto --target x86_64-unknown-linux-gnu
```
