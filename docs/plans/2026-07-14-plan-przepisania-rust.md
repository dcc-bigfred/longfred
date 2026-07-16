# WiTcontroller → LongFred rewrite plan (Rust, ESP32-C6, async/embassy)

Date: 2026-07-14
Source repo: [flash62au/WiTcontroller](https://github.com/flash62au/WiTcontroller) (cloned to `.tmp/WiTcontroller`)
Target directory: `longfred/`

---

## 1. Project goal

Rewrite the existing WiTcontroller firmware (Arduino/C++, classic ESP32) into a custom
**LongFred** firmware in **Rust**, for **ESP32-C6**, with emphasis on:

- **async / no-std** — `esp-hal` + **embassy** framework, without `std` library,
- **WiFi 6 (802.11ax)** on ESP32-C6 — minimizing communication latency,
- maximum performance and low control latency (WiThrottle over TCP),
- clean, modular code (as opposed to a single 3885-line `.ino` file).

> Note on work order: **this document is only the rewrite plan.** Adaptation to
> a specific ESP32-C6 board (pins, peripherals), WiFi 6 optimizations and new feature
> development are **separate, later tasks** — marked in section 8.

---

## 2. What the original does (source analysis)

WiThrottle handheld DCC controller. Talks to a WiThrottle server (JMRI,
DCC-EX EX-CommandStation, LnWi, etc.) via TCP text WiThrottle protocol.

### Source structure

| File | Lines | Contents |
|------|-------|-----------|
| `WiTcontroller.ino` | 3885 | All logic: setup/loop, state, menu, OLED handling, WiFi, keypad, encoder, protocol delegate |
| `WiTcontroller.h` | 213 | `extern` global state declarations + function prototypes |
| `actions.h` | 83 | Action constants (FUNCTION_0..31, SPEED_*, DIRECTION_*, POWER_*, THROTTLE_*, CUSTOM_* …) |
| `config_keypad_etc.h` | 63 | Default encoder and 4×3 keypad pins, `Keypad` object |
| `config_buttons_example.h` | 587 | Key/button to action mapping, UI options, menu definitions |
| `config_network_example.h` | 99 | SSID/password list, country code, WiFi scan options, default IP:port |
| `static.h` | 1176 | Default texts (i18n), menu definitions, messages |
| `language_*.h` | ~299 | Translations (DE/IT/NL/CN) |
| `Pangodream_18650_CL.*` | ~270 | Li-Ion battery voltage measurement |

### External dependencies (Arduino libraries)

- `WiThrottleProtocol` — WiThrottle protocol client (parser + delegate). **To be rewritten.**
- `U8g2` — OLED SSD1306/SH1106 128×64 driver (I2C). → in Rust: `ssd1306` + `embedded-graphics`.
- `Keypad` — 4×3 matrix scanning. → custom GPIO driver.
- `AiEsp32RotaryEncoder` — KY-040/EC11 encoder on interrupts. → custom GPIO/pcnt driver.
- `WiFi.h`, `ESPmDNS.h`, `Preferences.h` — WiFi STA, mDNS discovery, NVS.

### Key functions and flows (to replicate)

1. **Startup / WiFi**: SSID scan (`WiFi.scanNetworks`), SSID/password selection/entry
   (via encoder), password stored in NVS, STA connection.
2. **Server discovery**: mDNS `queryService("withrottle","tcp")`, server list,
   auto-connect, DCC-EX AP IP/port guess (192.168.4.1:2650), manual IP:port entry.
3. **WiThrottle protocol** (`wiThrottleProtocol` object + `MyDelegate`):
   - outgoing: `connect`, `addLocomotive`, `releaseLocomotive`, `stealLocomotive`,
     `setSpeed`, `setDirection`, `getSpeed/getDirection`, `setTurnout`, `setRoute`,
     `emergencyStop`, `sendCommand`, `requireHeartbeat`, heartbeat/`check()`.
   - incoming (delegate callbacks): `heartbeatConfig`, `receivedVersion`,
     `receivedServerDescription`, `receivedMessage`, `receivedAlert`,
     `receivedSpeedMultiThrottle`, `receivedDirectionMultiThrottle`,
     `receivedFunctionStateMultiThrottle`, `receivedRosterFunctionListMultiThrottle`,
     `receivedTrackPower`, `receivedRoster/Turnout/RouteEntries` + `…Entry`,
     `receivedUnknownCommand`.
4. **Domain model**: up to 6 "throttles", each with locomotive list (consist/MU),
   speed (0–126), direction, 32 functions, function labels from roster.
   Registries: roster (≤70), turnouts (≤60), routes (≤60).
5. **Input**: 4×3 keypad (menu `*` … `#`, direct actions 0–9),
   encoder (speed + list selection), encoder button, up to 11 additional buttons.
6. **OLED 128×64 UI**: screens (SSID scan, SSID list, server list, throttle,
   menu, roster/turnout/route/function lists, password entry), battery icon.
7. **Persistence (NVS)**: SSID, passwords, saved locos for auto-reacquire.
8. **Power**: battery measurement, deep sleep (wake on encoder button),
   auto-shutdown on inactivity.

---

## 3. Target technology stack (Rust, no-std)

| Layer | Crate (proposal) | Notes |
|---------|--------------------|-------|
| HAL / runtime | `esp-hal` (target `riscv32imac`, ESP32-C6) | no-std, async |
| Executor | `embassy-executor`, `embassy-time` | async tasks |
| WiFi / radio | `esp-wifi` (+ `esp-radio`) | STA, WiFi 6 on C6 |
| TCP/IP stack | `embassy-net` (smoltcp) | DHCP, TCP socket |
| mDNS | `edge-mdns` or custom minimal query | discovery `_withrottle._tcp` |
| OLED | `ssd1306` + `embedded-graphics` | I2C async (`embassy` I2C) |
| Fonts/UI | `embedded-graphics`, `u8g2-fonts` (optional) | screen replication |
| Persistence | `sequential-storage` / `esp-storage` + `embedded-storage` | NVS/Preferences substitute |
| Logging | `defmt` + `esp-println` or `log` | debug via UART/RTT |
| Allocation | preferably **no heap** (`heapless`: `String`, `Vec`) | `esp-alloc` if necessary |
| Concurrency | `embassy-sync` (Channel, Signal, Mutex) | inter-task communication |
| Errors | `thiserror`-no-std / manual enums | — |

**Architecture principle:** instead of C++ "delegate + global state" pattern, use
**embassy tasks communicating via channels** (`embassy-sync::Channel`) and
shared state in `Mutex`/`Signal`. Protocol parser emits events (delegate callback equivalents)
to a channel; UI task and domain task consume them.

### Module sketch `longfred/src/`

```
src/
  main.rs            // init HAL, spawn embassy tasks
  config/            // config_* equivalent (pins, mappings, network) – compile-time
  input/
    keypad.rs        // 4x3 matrix scan
    encoder.rs       // encoder + button
    buttons.rs       // additional buttons
  ui/
    display.rs       // OLED driver + buffer
    screens.rs       // screens (throttle, menu, lists, password…)
    fonts.rs / i18n.rs
  net/
    wifi.rs          // STA, scan, connect
    mdns.rs          // WiThrottle server discovery
    tcp.rs           // TCP connection
  withrottle/
    protocol.rs      // outgoing command construction
    parser.rs        // incoming message parser → Event
    client.rs        // client task (I/O + heartbeat)
    events.rs        // Event enum (callback equivalents)
  domain/
    model.rs         // Throttle, Loco, Consist, Roster, Turnouts, Routes
    actions.rs       // actions.h equivalent
    state.rs         // shared application state
  storage/
    prefs.rs         // NVS: SSID, passwords, saved locos
  power/
    battery.rs
    sleep.rs
```

---

## 4. Rewrite stages

Each stage ends with a **working, testable artifact** (compile + flash).
Stages 1–10 cover the rewrite itself; stage 11+ (C6 tuning, optimizations, features)
are separate future tasks.

### Stage 0 — Foundation / toolchain
- Cargo project init (`no_std`, `no_main`) in `longfred/`.
- RISC-V toolchain for ESP32-C6 (`espup`/`rustup` target, `espflash`/`probe-rs`).
- `Cargo.toml`: `esp-hal`, `embassy-executor`, `embassy-time`, `esp-println`/`defmt`.
- Minimal "blink" + UART log as smoke test.
- **DoD:** firmware starts on C6, blinks LED, logs to serial.

### Stage 1 — Application skeleton and configuration
- Module structure (section 3).
- Move `actions.h` → `domain/actions.rs` (`Action` enum).
- Compile-time config (pins, key mappings, SSID list) — equivalent of
  `config_keypad_etc.h` / `config_buttons.h` / `config_network.h`.
- Size constant definitions (roster, turnouts, routes) on `heapless`.
- **DoD:** project compiles with full module structure (stubs).

### Stage 2 — Input peripherals (no network)
- `input/keypad.rs`: async 4×3 matrix scan with debounce (embassy task).
- `input/encoder.rs`: encoder (GPIO/pcnt) + button, event emission to channel.
- `input/buttons.rs`: additional buttons (optional).
- Input events → `embassy-sync::Channel<InputEvent>`.
- **DoD:** key presses and encoder rotations log to serial.

### Stage 3 — OLED display
- `ui/display.rs`: init SSD1306 128×64 via I2C (async), `embedded-graphics`.
- `ui/fonts.rs` + `ui/i18n.rs`: texts (EN first, i18n like `static.h`/`language_*`).
- Render simple startup screen (appName + version + status).
- **DoD:** startup screen visible on OLED; text/icon drawing test.

### Stage 4 — WiFi (STA) + network stack
- `net/wifi.rs`: `esp-wifi` STA, SSID scan, password connect.
- `embassy-net`: DHCP, IP acquisition.
- SSID selection integration with UI + password entry via encoder (like original).
- **DoD:** device connects to WiFi and gets IP; SSID selected from UI.

### Stage 5 — Server discovery (mDNS)
- `net/mdns.rs`: query `_withrottle._tcp`, parse responses (host/IP/port/TXT).
- Found server list in UI, auto-connect, DCC-EX AP guess,
  manual IP:port entry.
- **DoD:** WiThrottle server list displayed; server selected for connection.

### Stage 6 — WiThrottle protocol: parser + events
- `withrottle/parser.rs`: incoming message parser → `enum Event`
  (equivalents of all `MyDelegate` callbacks).
- `withrottle/protocol.rs`: outgoing command construction (multi-throttle,
  loco, speed, direction, turnout, route, e-stop, heartbeat, raw command).
- `withrottle/events.rs`: `Event` definition + channel to domain/UI layer.
- Parser unit tests on protocol samples (host, no hardware).
- **DoD:** parser passes tests on example protocol frames.

### Stage 7 — TCP client + protocol loop
- `net/tcp.rs` + `withrottle/client.rs`: async task — TCP connection,
  read/write, `requireHeartbeat`, response watchdog + reconnect.
- Connect parser (in) and command builder (out) to socket.
- **DoD:** connection to real server (JMRI/DCC-EX), heartbeat, version/roster reception.

### Stage 8 — Domain model and control logic
- `domain/model.rs`: `Throttle` × ≤6, `Consist`/`Loco`, roster, turnouts, routes,
  functions (32) + labels, speed/direction, speed multiplier.
- `domain/state.rs`: shared state (`Mutex`/`Signal`), updated by events.
- Action logic: acquire/release/steal, speed up/down/stop/e-stop, direction,
  next throttle, power on/off, turnout throw/close, route set, functions (latching).
- **DoD:** locomotive control (speed/direction/functions) works end-to-end.

### Stage 9 — Menu, screens and full UI
- `ui/screens.rs`: menu state machine (`*`…`#`), throttle screen (as in README),
  roster/turnout/route/function lists with pagination, password screen, broadcast messages.
- Key mapping 0–9 to default/configurable domain actions (from config).
- **DoD:** full set of screens and menu replicated; navigation like original.

### Stage 10 — Persistence, battery, sleep, i18n
- `storage/prefs.rs`: read/write SSID, passwords, saved locos (`sequential-storage`).
- `power/battery.rs`: voltage measurement (ADC), battery icon/percentage.
- `power/sleep.rs`: deep sleep + button wake, auto-shutdown on inactivity.
- Translation completion (DE/IT/NL/CN) — optional.
- **DoD:** functional parity with original WiTcontroller on target board.

---

## 5. Mapping: C++ → Rust (conceptual summary)

| Original (C++/Arduino) | Equivalent (Rust/embassy) |
|------------------------|----------------------------|
| `setup()` + `loop()` | `#[esp_hal::main]` init + spawn tasks, each task has its own loop |
| Global `extern` state (`WiTcontroller.h`) | `domain/state.rs` in `Mutex`/`Signal` (`embassy-sync`) |
| `MyDelegate` (callbacks) | `enum Event` + `Channel<Event>` |
| `wiThrottleProtocol.*` | `withrottle/` module (protocol/parser/client) |
| `String` (Arduino) | `heapless::String<N>` (no heap) |
| `[maxRoster]` arrays etc. | `heapless::Vec<_, N>` |
| `Keypad`, `AiEsp32RotaryEncoder` | custom `input/*` tasks with GPIO |
| `U8g2` | `ssd1306` + `embedded-graphics` |
| `Preferences` (NVS) | `storage/prefs.rs` (`sequential-storage`) |
| `#define` from config_* | `config/` (const/compile-time features) |
| `delay()`, `millis()` | `embassy_time::{Timer, Instant}` |

---

## 6. Risks / points to investigate

- **WiFi 6 in `esp-wifi`**: check how much the crate exposes HE/TWT/OFDMA features on C6
  (part hardware-negotiated). Latency gains may require QoS/power-save settings.
- **mDNS in no-std**: `edge-mdns` vs custom minimal query client.
- **No `String`/heap**: choose `heapless` sizes (SSID, passwords, loco names, messages).
- **Encoder on C6**: PCNT peripheral use (if available in `esp-hal`) vs GPIO+interrupt.
- **ESP32-C6 pins ≠ classic ESP32**: pin mapping is a separate task (stage 11).
- **Fonts/i18n CJK**: Chinese characters require larger fonts — low priority.

---

## 7. Verification / tests

- Unit tests for **protocol parser** and **command builder** (host, `cargo test`
  in separate crate/`no_std`-friendly) — no hardware.
- Hardware integration tests after stages 4, 7, 8 (WiFi, protocol, control).
- `defmt`/serial log as main diagnostic tool.

---

## 8. Follow-up tasks (outside this plan — to do later)

1. **Adaptation to specific ESP32-C6 board** — pin mapping, peripheral choice,
   alternate display/encoder.
2. **WiFi 6 / latency optimizations** — TWT/power-save, QoS, `embassy-net`/smoltcp tuning,
   WiThrottle command RTT minimization, batching.
3. **Feature development** — beyond original parity (new actions, better UI,
   auto-reconnect, etc.).

---

## 9. Next step

After plan approval I propose starting with **Stage 0** (Cargo project bootstrap for
ESP32-C6 + smoke test), then **Stage 6** (WiThrottle protocol parser with tests),
because the protocol is the heart of the application and can be developed/tested independently of hardware.
