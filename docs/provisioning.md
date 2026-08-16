# LongFred programming / pairing mode

All hardware variants share the same Soft-AP provisioning API. Firmware can also be uploaded over HTTP while the throttle is already on the layout Wi‑Fi (STA), from the Extras menu.

## Entering Soft-AP mode

On OLED variants, **Stop during the 2 s boot splash** also enters programming mode (in addition to the 8 s chord).

| Variant | Chord (hold 8 s) | Boot splash Stop | Auto if no Wi‑Fi creds |
|---------|------------------|------------------|------------------------|
| longfred-standard / mini | Shift1 + Stop | yes | no |
| markwtech | `*` + Stop | yes | no |
| heiko-wifred | Shift + Stop | no (no OLED) | **yes** |

Firmware sets `programming_mode` in NVS and soft-resets (except auto-pair at boot, which skips STA bring-up).

On OLED variants the Soft-AP UI is two steps:

1. SSID (`longfred_prog_XXXXXX`) and “connected?” — **left** cancels (clears the flag and reboots to normal boot), **right** continues.
2. `http://192.168.0.1/` — 128×64 shows a QR code with the URL under it; 128×32 shows the URL only. **Left** returns to step 1. **Stop** / **E-Stop** exits from either step.

heiko-wifred has no OLED (LED pairing pattern); **Stop** still exits.

## Network (Soft-AP)

| Setting | Value |
|---------|-------|
| SSID | `longfred_prog_XXXXXX` (6-char MAC suffix) |
| Security | open |
| AP IP | `192.168.0.1/24` |
| DHCP | pool `192.168.0.50`–`192.168.0.200`, lease ~1 h, gateway/DNS `192.168.0.1` |

`192.168.0.2` is **outside** the DHCP pool so `wireless-programmer` can keep using that static address.

### Phone / laptop

1. Join `longfred_prog_XXXXXX`
2. Wait for DHCP (or set a static IPv4 in `192.168.0.0/24`, not `.1` / `.2`)
3. Open `http://192.168.0.1/`

### wireless-programmer

Associates open, assigns `192.168.0.2/24`, talks HTTP to `192.168.0.1:80` (driver id `longfred`).

## Firmware update over HTTP

Use the **app image** (`*.app.bin` from CI — `espflash save-image` **without** `--merge`). Merged flash dumps are rejected.

The first install of the dual-slot partition table (`partitions.csv`) must be done over **USB** (`espflash flash`, or `wireless-programmer update-firmware --mode usb`). Later updates can use HTTP OTA or USB.

```bash
# First install (ELF + partition table), or a merged `.bin` from CI:
wireless-programmer update-firmware --mode usb --port /dev/ttyUSB0 \
  --file dist/longfred-markwtech-esp32c6.elf --partition-table partitions.csv
```

```bash
curl -T dist/longfred-markwtech-esp32c6.app.bin \
  http://192.168.0.1/api/v1/firmware
```

### Soft-AP path

Upload from the pairing page or `POST /api/v1/firmware`. After a successful write the device reboots **back into Soft-AP** (`programming_mode` stays set) so you can confirm the new version.

### STA / LAN path (not heiko-wifred)

On variants with a menu: **Extras → Firmware update** (encoder; digits 0–9 stay on the other extras items). OK toggles HTTP on the layout IPv4, port 80. The device announces `_longfred-ota._tcp.local`. Open `http://<sta-ip>/` and upload `.app.bin`. **Back** (and sleep) turn HTTP and mDNS off. After STA OTA the device reboots onto layout Wi‑Fi (`programming_mode` stays false).

heiko-wifred has no menu — firmware OTA is Soft-AP only.

## HTTP API

### `GET /`

Static HTML configuration page (inline CSS/JS), including firmware file upload.

On STA this page is served only while Firmware update HTTP is enabled. `PUT` settings and `POST …/programming-mode/off` are Soft-AP only.

### `GET /api/v1/settings`

Returns device info (including `device.variant` and `firmware.version`), Wi‑Fi SSIDs plus the last-used `wifi.password`, BigFred login and PIN (`bigfred.pin`, `pin_set` remains for compatibility), roster, roster mode.

### `PUT /api/v1/settings`

Soft-AP only. Partial JSON body:

```json
{
  "wifi": { "ssid": "layout", "password": "secret" },
  "bigfred": { "login": "user", "pin": "1234" },
  "rosterMode": "static",
  "roster": [{ "addr": "3", "name": "SHUNT" }]
}
```

All top-level fields optional. Persisted to NVS.

> **Note:** the server requires a `Content-Length` header and does **not**
> support `Transfer-Encoding: chunked`. Settings bodies are limited to 1536 bytes
> (`400 body too large`). Firmware upload streams the body and is not subject to
> that cap (timeout ~120 s; image must fit the inactive OTA slot, 0x3C0000 bytes).

### `POST /api/v1/firmware`

Raw `application/octet-stream` ESP32-C6 **app** image (`Content-Length` required). Validates header magic `0xE9` and chip id `0x000D`, writes the inactive OTA slot, responds 200, then soft-resets.

### `POST /api/v1/programming-mode/off`

Soft-AP only. Clears the programming flag, responds 200, soft-resets after ~500 ms.

## Cancel on device

On OLED variants, **left** on the first Soft-AP screen (or **Stop** / **E-Stop** on either screen) clears the flag and reboots into normal operation. **Right** on the first screen shows the URL / QR.
