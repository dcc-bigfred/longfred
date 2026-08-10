# LongFred

Wireless physical throttle client for BigFred (WiThrottle / Z21).

## Hardware variants

Build-time Cargo features (mutually exclusive):

| Feature | Description |
|---------|-------------|
| `variant-longfred-standard` (default) | OLED 128×64, MCP23017×2, 5-way + F-keys + encoder |
| `variant-longfred-mini` | Same as standard, OLED 128×32 |
| `variant-markwtech` | Keypad + 2.42" OLED, WiTcontroller-style |
| `variant-heiko-wifred` | Headless wiFred-style (LEDs + pot), Wi‑Fi config only |

Docs: [docs/hardware/](docs/hardware/), provisioning: [docs/provisioning.md](docs/provisioning.md).

```bash
cargo build -p longfred-firmware --release --bin longfred
cargo build -p longfred-firmware --release --bin longfred \
  --no-default-features --features variant-longfred-mini
```

## Host tests

```bash
cargo test -p longfred-proto --target x86_64-unknown-linux-gnu
```
