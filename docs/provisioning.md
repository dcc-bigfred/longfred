# LongFred programming / pairing mode

All hardware variants share the same Soft-AP provisioning API.

## Entering

| Variant | Chord (hold 8 s) | Auto if no Wi‑Fi creds |
|---------|------------------|------------------------|
| longfred-standard / mini | Shift1 + Stop | no |
| markwtech | `*` + Stop | no |
| heiko-wifred | Shift + Stop | **yes** |

Firmware sets `programming_mode` in NVS and soft-resets (except auto-pair at boot, which skips STA bring-up).

## Network

| Setting | Value |
|---------|-------|
| SSID | `longfred_prog_XXXXXX` (6-char MAC suffix) |
| Security | open |
| AP IP | `192.168.0.1/24` |
| DHCP | **none** — client must use a static address |

### Phone / laptop (manual static IP)

1. Join `longfred_prog_XXXXXX`
2. Set static IPv4: address `192.168.0.50`, mask `255.255.255.0`, gateway `192.168.0.1`
3. Open `http://192.168.0.1/`

### wireless-programmer

Associates open, assigns `192.168.0.2/24`, talks HTTP to `192.168.0.1:80` (driver id `longfred`).

## HTTP API

### `GET /`

Static HTML configuration page (inline CSS/JS).

### `GET /api/v1/settings`

Returns device info (including `device.variant`), Wi‑Fi SSID (no password), BigFred login (no PIN), roster, roster mode.

### `PUT /api/v1/settings`

Partial JSON body:

```json
{
  "wifi": { "ssid": "layout", "password": "secret" },
  "bigfred": { "login": "user", "pin": "1234" },
  "rosterMode": "static",
  "roster": [{ "addr": "3", "name": "SHUNT" }]
}
```

All top-level fields optional. Persisted to NVS.

### `POST /api/v1/programming-mode/off`

Clears the programming flag, responds 200, soft-resets after ~500 ms.

## Cancel on device

Press **Stop** / **EStop** while in pairing UI to clear the flag and reboot into normal operation.
