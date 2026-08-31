# Wi-Fi Roaming — LongFred

Fast (<1 s) access-point transition for LongFred controllers on ESP32-C6,
with TP-Link Omada infrastructure configuration (OC200 + EAP610/613/650 + TL-SF1006P).

## 1. Goal and scope

LongFred is a wireless DCC throttle on ESP32-C6 (Wi-Fi 6, 2.4 GHz).
On a large layout with many controllers, the throttle needs seamless
access-point roaming without losing control and with recovery
time under one second.

This document describes:

- the required Omada infrastructure configuration,
- the `RadioConfig` firmware parameters (14 fields),
- the IPv4 pinning mechanism,
- the upstream 802.11k/v/r support status in esp-radio,
- how to edit settings (OLED, HTTP, factory reset),
- troubleshooting,
- measuring roam time.

Target audience: layout operators and Omada network configurators.

## 2. Infrastructure layer

### Required Omada configuration

| Setting | Value | Notes |
|---|---|---|
| SSID | one, shared across all EAPs | Otherwise IP pinning and 802.11r make no sense |
| VLAN | one, shared across all EAPs | Same L2 on the TL-SF1006P |
| 802.11r (FT-PSK) | enabled, over-the-air | WLAN profile in Omada |
| 802.11k (RRM) | enabled | WLAN profile in Omada |
| 802.11v (BTM) | enabled | WLAN profile in Omada |
| Band steering | disabled | ESP32-C6 is 2.4 GHz only |
| TX power | -12 to -15 dBm at the edge | Coverage zones must overlap heavily |
| DHCP reservations | per MAC, for all controllers | Removes the IP-conflict risk from pinning |

### Topology

```
Internet
  |
OC200 (Omada controller)
  |
TL-SF1006P (PoE switch)
  |
  +--+--+--+--
  |  |  |  |
EAP613 EAP613 EAP650 EAP610
```

All EAPs on the same L2 (TL-SF1006P without routing between them).

## 3. `RadioConfig` parameters

| Parameter | Range | Default | Description |
|---|---|---|---|
| `roam_enabled` | bool | false | Master switch. false = sticky client (holds AP until signal loss). true = `RoamEngine` decides based on RSSI |
| `rrm_enabled` | bool | true | 802.11k (RRM). No-op until Tier B lands in esp-radio |
| `btm_enabled` | bool | true | 802.11v (BTM). No-op until Tier B lands in esp-radio |
| `ft_enabled` | bool | true | 802.11r (FT). No-op until Tier C lands in esp-radio |
| `power_save_off` | bool | true | Energy saving off (default): modem PS `None`. Uncheck / `false` enables DTIM `Minimum` power-save |
| `enable_11ax` | bool | true | 802.11ax (OFDMA) on 2.4 GHz |
| `roam_rssi_threshold` | -90..=-50 | -72 | Threshold in -dBm below which `RoamEngine` starts looking for a better AP |
| `roam_hysteresis_db` | 3..=20 | 8 | Minimum RSSI delta (dB) between current and candidate for a roam to fire. Ping-pong guard |
| `roam_debounce_samples` | 1..=10 | 3 | Consecutive samples below threshold before `RoamEngine` reacts. Transient-dip guard |
| `roam_scan_interval_s` | 1..=60 | 10 | Minimum gap (s) between roam-initiated scans |
| `roam_sample_ms` | 100..=2000 | 250 | RSSI sampling period (ms) |
| `ip_pinning` | bool | true | Pin IPv4 after link-down (see section 4) |
| `ip_pin_max_gap_s` | 5..=3600 | 120 | Unpin after this many seconds of link-down gap |
| `dhcp_discover_timeout_s` | 1..=30 | 2 | smoltcp DISCOVER timeout (default 10) |

### Threshold vs hysteresis

These are independent levers. **Threshold** decides *when to look*; **hysteresis**
decides *whether the candidate is worth switching to*.

- `roam_rssi_threshold`: while the current AP's RSSI is **≥ threshold**,
  `RoamEngine` does nothing. A scan starts only after RSSI stays **below**
  threshold for `roam_debounce_samples` consecutive samples.
- `roam_hysteresis_db`: the candidate must satisfy

  ```
  candidate.rssi  ≥  current.rssi  +  hysteresis
  ```

  It is not a second absolute RSSI floor. It compares the candidate against
  whatever you have *now*, so a few-dB fade on the coverage edge does not
  ping-pong between EAPs.

Example with hysteresis = **8** and threshold = **−72**:

| Current AP | Candidate | Delta | Roam? |
|---|---|---|---|
| −78 dBm | −70 dBm | +8 dB | yes (exactly 8) |
| −78 dBm | −72 dBm | +6 dB | no |
| −78 dBm | −65 dBm | +13 dB | yes |
| −60 dBm | −50 dBm | +10 dB | no — scan never starts (still above threshold) |

On the OLED, RSSI is entered as a magnitude (`72` → `−72`); hysteresis is an
unsigned dB value.

## 4. IPv4 pinning

### Mechanism

After a link-down (roam, coverage loss, AP restart) the firmware pins the previous
address as static instead of re-querying DHCP from scratch. On link return the address
is ready immediately — zero seconds for DORA.

### Validation

- If the SSID differs from the remembered one — unpin immediately.
- Otherwise a single ICMP echo to the gateway with a short timeout.
  Failure means a different subnet or VLAN — unpin and allow a normal DORA.

### Watchdog

After `ip_pin_max_gap_s` seconds of absence we unpin, to avoid returning to a
completely different network after a long absence.

### Why DHCP reservations per MAC are recommended

The pinned address is not renewed for the entire session. With a short lease
the server may assign it to someone else. Reservations remove this problem at the
source: the pinned address is by definition the one the server would have
assigned anyway.

### Why we do not renew leases in the background

Returning to DHCP kills open TCP sessions — the address disappears
immediately, before DORA completes. Lease hygiene is moved to DHCP reservations.

### Limitations

- Reboot and deep sleep clear `LAST_LEASE` (RAM) — after wake-up there is a normal DORA.
- Static IP from NVS makes the mechanism moot — without a DHCP socket there is nothing to pin.

## 5. Upstream 802.11k/v/r status in esp-radio

| Tier | What | Status |
|---|---|---|
| A | BSSID lock + neighbor scan | Works today, no library changes |
| B | 802.11k/v (RRM/BTM) | Needs a PR to esp-radio + esp-wifi-sys. Deployed via a fork with `[patch.crates-io]` |
| C | 802.11r (FT) | Additionally requires a supplicant blob rebuild. Requires a flash/RAM budget measurement |

Until Tier B/C lands, the `rrm_enabled`/`btm_enabled`/`ft_enabled` flags are no-op (default `true`, but esp-radio ignores them). `roam_enabled` always works (Tier A).

## 6. Editing settings

### OLED

Wi-Fi settings → Radio settings → select field → edit:

- bool field: list `1: On` / `2: Off`
- numeric field: digit keyboard, clamped to range on commit
- OK → "Radio saved" message → back to list

### HTTP in programming mode

`http://192.168.4.1/` → "Radio" section → Save → Exit (reboot).

### Factory reset

Via Soft-AP — clearing credentials also resets `RadioConfig` to defaults.

## 7. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Controller does not roam despite weak RSSI | `roam_enabled` off, `roam_rssi_threshold` too low, Tier B/C not deployed | Enable `roam_enabled`, raise threshold, deploy Tier B/C |
| Roam works but TCP session drops for a long time | `ip_pinning` off, no DHCP reservations, `dhcp_discover_timeout_s` too high | Enable `ip_pinning`, add reservations, lower timeout |
| Ping-pong between APs | `roam_hysteresis_db` and `roam_debounce_samples` too low | Increase values |
| Controller loses address after a long gap | Intended (watchdog `ip_pin_max_gap_s`) | Increase parameter or disable pinning |
| Different IP after roam than before | No DHCP reservation per MAC, or different VLAN on AP | Check Omada configuration |

## 8. Measuring roam time

### `wifi` logs

```
wifi connected              BSSID=aa:bb:cc:dd:ee:01 channel=6
wifi roam: rssi=-78 -> scan
wifi roam: target BSSID=aa:bb:cc:dd:ee:02 channel=11
wifi connected              BSSID=aa:bb:cc:dd:ee:02 channel=11
net ready: ip=192.168.1.42
```

Time from `rssi=-78` to `net ready` is the roam + recovery time.

### Diagnostics screen

RSSI chart (sampled every `roam_sample_ms`), ping RTT to the command station.
