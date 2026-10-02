# LongFred v1 (custom PCB)

ESP32-C6 QFN-40 handheld throttle. Schematic/PCB: `longfred-hardware/longfred-v1/`. Pin and MCP map: `longfred-hardware/plans/004-schematic.md`.

## Features

| Item | Value |
|------|-------|
| MCU | ESP32-C6 QFN-40 + W25Q64 + 40 MHz + u.FL |
| Display | SSD1306 128×32 I2C `@ 0x3C` (OLED 0.91" on J_DISP 1–4). E-ink SPI on J_DISP 5–10 unused in this firmware. |
| I/O | 2× MCP23017 (0x20, 0x21) |
| Speed | EC11 encoder A/B/SW |
| Cargo feature | `variant-longfred-v1` |
| Programming chord | **Shift1 + Back** held 8 s (Back maps to Stop) |

## Pin map

| Function | GPIO |
|----------|------|
| I2C SDA / SCL | 6 / 7 |
| Encoder A / B / SW (wake) | 1 / 2 / 0 |
| Battery ADC | 4 |
| VBUS sense | 10 |
| MCP INTA | 11 |
| USB D− / D+ | 12 / 13 |
| EPD MOSI/SCK/CS/DC/RST/BUSY | 18 / 19 / 20 / 21 / 22 / 23 |

MCP U5 (0x20): GPA0–6 = F0–F6; GPB0–6 = F7, F8, Shift1, Shift2, EStop, Menu, Back. GPA7/GPB7 unused (errata).

MCP U6 (0x21): GPA0–4 = Joy U/D/L/R/center; GPA5 = direction SPDT.

## First flash (USB Serial/JTAG)

Native USB-C on the C6 (GPIO12/13) enumerates as CDC (`/dev/ttyACM0` typically). First install needs the dual-slot partition table:

```bash
make flash VARIANT=longfred-v1
# or:
espflash flash --port /dev/ttyACM0 --partition-table partitions.csv \
  target/longfred-v1/riscv32imac-unknown-none-elf/release/longfred
```

Later updates: Soft-AP `longfred_prog_XXXXXX` or Extras → Firmware update. See [provisioning.md](../provisioning.md).

## Bring-up tests (assembled board)

1. USB enumerates; `espflash board-info`.
2. Firmware boots; OLED splash 128×32.
3. I2C scan: 0x20, 0x21, and 0x3C with OLED fitted.
4. F0–F8, Shift1/2, EStop, Menu, Back, joystick, direction SPDT.
5. Encoder changes speed; SW wakes from deep sleep (GPIO0).
6. USB present → charging LED; VBAT ADC moves with cell voltage.
