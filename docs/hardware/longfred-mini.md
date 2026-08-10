# LongFred Mini

Same hardware family as [LongFred Standard](longfred-standard.md), but with a **0.91" SSD1306 128×32** OLED.

| Item | Value |
|------|-------|
| Cargo feature | `variant-longfred-mini` |
| Display | SSD1306 128×32 I2C `@ 0x3C` |
| Controls / expanders / encoder | Identical to standard |
| Programming chord | **Shift1 + Stop** 8 s |

## Software sharing

Pinout, `ControlSurface`, `NavProfile`, and Shift layers live in one module (`board/variants/longfred_family.rs`). The only compile-time difference is `DisplayGeometry` (128×32 compact layout).

## Compact throttle layout

| Band | Y | Content |
|------|---|--------|
| Speed | 0–12 | Speed `8×13`, direction + loco `6×10` |
| Info | 13–22 | Button/footer line `6×10` |
| Functions | 25–30 | F0–F28 strip `4×6` |

Menu grids use 3 rows × 2 columns (~6 lines) instead of 12.

## Wiring

Same as [longfred-standard.md](longfred-standard.md); swap the OLED for a 128×32 module.

## BOM delta

- OLED 0.91" SSD1306 128×32 I2C (e.g. Allegro 0.91" modules) instead of 128×64
