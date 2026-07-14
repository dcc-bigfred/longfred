# Plan przepisania WiTcontroller → LongFred (Rust, ESP32-C6, async/embassy)

Data: 2026-07-14
Repo źródłowe: [flash62au/WiTcontroller](https://github.com/flash62au/WiTcontroller) (sklonowane do `.tmp/WiTcontroller`)
Cel katalogu: `longfred/`

---

## 1. Cel projektu

Przepisać istniejący firmware WiTcontroller (Arduino/C++, ESP32 klasyczny) na własny
firmware **LongFred** w **Rust**, dla **ESP32-C6**, z naciskiem na:

- **async / no-std** — `esp-hal` + framework **embassy**, bez biblioteki `std`,
- **WiFi 6 (802.11ax)** na ESP32-C6 — minimalizacja opóźnień komunikacyjnych,
- maksymalną wydajność i niskie latency sterowania (WiThrottle over TCP),
- czysty, modularny kod (w przeciwieństwie do jednego pliku `.ino` na 3885 linii).

> Uwaga o kolejności prac: **ten dokument to tylko plan przepisania.** Dostosowanie do
> konkretnej płytki ESP32-C6 (piny, peryferia), optymalizacje WiFi 6 i rozwój nowych
> funkcji to **osobne, późniejsze zadania** — zaznaczone w sekcji 8.

---

## 2. Co robi oryginał (analiza źródła)

WiThrottle-owy, ręczny kontroler DCC. Rozmawia z serwerem WiThrottle (JMRI,
DCC-EX EX-CommandStation, LnWi itd.) przez TCP tekstowym protokołem WiThrottle.

### Struktura źródła

| Plik | Linie | Zawartość |
|------|-------|-----------|
| `WiTcontroller.ino` | 3885 | Cała logika: setup/loop, stan, menu, obsługa OLED, WiFi, klawiatura, enkoder, delegate protokołu |
| `WiTcontroller.h` | 213 | Deklaracje `extern` stanu globalnego + prototypy funkcji |
| `actions.h` | 83 | Stałe akcji (FUNCTION_0..31, SPEED_*, DIRECTION_*, POWER_*, THROTTLE_*, CUSTOM_* …) |
| `config_keypad_etc.h` | 63 | Domyślne piny enkodera i klawiatury 4×3, obiekt `Keypad` |
| `config_buttons_example.h` | 587 | Mapowanie klawiszy/przycisków na akcje, opcje UI, definicje menu |
| `config_network_example.h` | 99 | Lista SSID/haseł, kod kraju, opcje skanowania WiFi, domyślne IP:port |
| `static.h` | 1176 | Teksty domyślne (i18n), definicje menu, komunikaty |
| `language_*.h` | ~299 | Tłumaczenia (DE/IT/NL/CN) |
| `Pangodream_18650_CL.*` | ~270 | Pomiar napięcia baterii Li-Ion |

### Zewnętrzne zależności (biblioteki Arduino)

- `WiThrottleProtocol` — klient protokołu WiThrottle (parser + delegate). **Do przepisania.**
- `U8g2` — sterownik OLED SSD1306/SH1106 128×64 (I2C). → w Rust: `ssd1306` + `embedded-graphics`.
- `Keypad` — skanowanie matrycy 4×3. → własny sterownik GPIO.
- `AiEsp32RotaryEncoder` — enkoder KY-040/EC11 na przerwaniach. → własny sterownik GPIO/pcnt.
- `WiFi.h`, `ESPmDNS.h`, `Preferences.h` — WiFi STA, mDNS discovery, NVS.

### Kluczowe funkcje i przepływy (do odwzorowania)

1. **Startup / WiFi**: skan SSID (`WiFi.scanNetworks`), wybór/wpisanie SSID i hasła
   (enkoderem), zapamiętanie hasła w NVS, łączenie STA.
2. **Discovery serwera**: mDNS `queryService("withrottle","tcp")`, lista serwerów,
   auto-connect, zgadywanie IP/portu dla DCC-EX AP (192.168.4.1:2650), ręczne wpisanie IP:port.
3. **Protokół WiThrottle** (obiekt `wiThrottleProtocol` + `MyDelegate`):
   - wychodzące: `connect`, `addLocomotive`, `releaseLocomotive`, `stealLocomotive`,
     `setSpeed`, `setDirection`, `getSpeed/getDirection`, `setTurnout`, `setRoute`,
     `emergencyStop`, `sendCommand`, `requireHeartbeat`, heartbeat/`check()`.
   - przychodzące (callbacki delegata): `heartbeatConfig`, `receivedVersion`,
     `receivedServerDescription`, `receivedMessage`, `receivedAlert`,
     `receivedSpeedMultiThrottle`, `receivedDirectionMultiThrottle`,
     `receivedFunctionStateMultiThrottle`, `receivedRosterFunctionListMultiThrottle`,
     `receivedTrackPower`, `receivedRoster/Turnout/RouteEntries` + `…Entry`,
     `receivedUnknownCommand`.
4. **Model domenowy**: do 6 "throttles", każdy z listą lokomotyw (konsist/MU),
   prędkość (0–126), kierunek, 32 funkcje, etykiety funkcji z rostera.
   Rejestry: roster (≤70), turnouts (≤60), routes (≤60).
5. **Wejście**: klawiatura 4×3 (menu `*` … `#`, bezpośrednie akcje 0–9),
   enkoder (prędkość + wybór z list), przycisk enkodera, do 11 przycisków dodatkowych.
6. **UI OLED 128×64**: ekrany (skan SSID, lista SSID, lista serwerów, throttle,
   menu, listy rostera/turnoutów/route/funkcji, wpisywanie hasła), ikona baterii.
7. **Trwałość (NVS)**: SSID, hasła, zapisane loco do auto-reacquire.
8. **Zasilanie**: pomiar baterii, deep sleep (wybudzanie przyciskiem enkodera),
   auto-wyłączenie po bezczynności.

---

## 3. Docelowy stack technologiczny (Rust, no-std)

| Warstwa | Crate (propozycja) | Uwagi |
|---------|--------------------|-------|
| HAL / runtime | `esp-hal` (target `riscv32imac`, ESP32-C6) | no-std, async |
| Executor | `embassy-executor`, `embassy-time` | zadania async |
| WiFi / radio | `esp-wifi` (+ `esp-radio`) | STA, WiFi 6 na C6 |
| Stos TCP/IP | `embassy-net` (smoltcp) | DHCP, TCP socket |
| mDNS | `edge-mdns` lub własny minimalny query | discovery `_withrottle._tcp` |
| OLED | `ssd1306` + `embedded-graphics` | I2C async (`embassy` I2C) |
| Fonty/UI | `embedded-graphics`, `u8g2-fonts` (opcjonalnie) | odwzorowanie ekranów |
| Trwałość | `sequential-storage` / `esp-storage` + `embedded-storage` | zamiennik NVS/Preferences |
| Logowanie | `defmt` + `esp-println` lub `log` | debug przez UART/RTT |
| Alokacja | najlepiej **bez heap** (`heapless`: `String`, `Vec`) | ew. `esp-alloc` jeśli konieczne |
| Współbieżność | `embassy-sync` (Channel, Signal, Mutex) | komunikacja między taskami |
| Błędy | `thiserror`-no-std / ręczne enum-y | — |

**Zasada architektury:** zamiast wzorca „delegate + globalny stan” z C++, użyjemy
**tasków embassy komunikujących się przez kanały** (`embassy-sync::Channel`) i
współdzielony stan w `Mutex`/`Signal`. Parser protokołu emituje zdarzenia (odpowiedniki
callbacków delegata) do kanału; task UI i task domeny je konsumują.

### Szkic modułów `longfred/src/`

```
src/
  main.rs            // init HAL, spawn tasków embassy
  config/            // odpowiednik config_* (piny, mapowania, sieć) – compile-time
  input/
    keypad.rs        // skan matrycy 4x3
    encoder.rs       // enkoder + przycisk
    buttons.rs       // przyciski dodatkowe
  ui/
    display.rs       // sterownik OLED + bufor
    screens.rs       // ekrany (throttle, menu, listy, hasło…)
    fonts.rs / i18n.rs
  net/
    wifi.rs          // STA, skan, łączenie
    mdns.rs          // discovery serwerów WiThrottle
    tcp.rs           // połączenie TCP
  withrottle/
    protocol.rs      // budowa komend wychodzących
    parser.rs        // parser komunikatów przychodzących → Event
    client.rs        // task klienta (I/O + heartbeat)
    events.rs        // enum Event (odpowiedniki callbacków)
  domain/
    model.rs         // Throttle, Loco, Consist, Roster, Turnouts, Routes
    actions.rs       // odpowiednik actions.h
    state.rs         // współdzielony stan aplikacji
  storage/
    prefs.rs         // NVS: SSID, hasła, zapisane loco
  power/
    battery.rs
    sleep.rs
```

---

## 4. Etapy przepisania

Każdy etap kończy się **działającym, testowalnym artefaktem** (kompilacja + wgranie).
Etapy 1–10 dotyczą samego przepisania; etap 11+ (dostrojenie C6, optymalizacje, features)
to osobne przyszłe zadania.

### Etap 0 — Fundament / toolchain
- Inicjalizacja projektu Cargo (`no_std`, `no_main`) w `longfred/`.
- Toolchain RISC-V dla ESP32-C6 (`espup`/`rustup` target, `espflash`/`probe-rs`).
- `Cargo.toml`: `esp-hal`, `embassy-executor`, `embassy-time`, `esp-println`/`defmt`.
- Minimalny „blink” + log przez UART jako smoke test.
- **DoD:** firmware startuje na C6, miga LED, loguje na serial.

### Etap 1 — Szkielet aplikacji i konfiguracja
- Struktura modułów (sekcja 3).
- Przeniesienie `actions.h` → `domain/actions.rs` (enum `Action`).
- Config compile-time (piny, mapowania klawiszy, lista SSID) — odpowiednik
  `config_keypad_etc.h` / `config_buttons.h` / `config_network.h`.
- Definicja stałych rozmiarów (roster, turnouts, routes) na `heapless`.
- **DoD:** projekt kompiluje się z pełną strukturą modułów (stub-y).

### Etap 2 — Peryferia wejścia (bez sieci)
- `input/keypad.rs`: async skan matrycy 4×3 z debounce (task embassy).
- `input/encoder.rs`: enkoder (GPIO/pcnt) + przycisk, emisja zdarzeń do kanału.
- `input/buttons.rs`: przyciski dodatkowe (opcjonalne).
- Zdarzenia wejścia → `embassy-sync::Channel<InputEvent>`.
- **DoD:** naciśnięcia klawiszy i obroty enkodera logują się na serial.

### Etap 3 — Wyświetlacz OLED
- `ui/display.rs`: init SSD1306 128×64 przez I2C (async), `embedded-graphics`.
- `ui/fonts.rs` + `ui/i18n.rs`: teksty (najpierw EN, i18n jak w `static.h`/`language_*`).
- Renderowanie prostego ekranu startowego (appName + wersja + status).
- **DoD:** ekran startowy widoczny na OLED; test rysowania tekstu/ikon.

### Etap 4 — WiFi (STA) + stos sieciowy
- `net/wifi.rs`: `esp-wifi` STA, skan SSID, łączenie z hasłem.
- `embassy-net`: DHCP, uzyskanie IP.
- Integracja wyboru SSID z UI + wpisywanie hasła enkoderem (jak w oryginale).
- **DoD:** urządzenie łączy się z WiFi i dostaje IP; SSID wybierany z UI.

### Etap 5 — Discovery serwera (mDNS)
- `net/mdns.rs`: query `_withrottle._tcp`, parsowanie odpowiedzi (host/IP/port/TXT).
- Lista znalezionych serwerów w UI, auto-connect, zgadywanie DCC-EX AP,
  ręczne wpisanie IP:port.
- **DoD:** lista serwerów WiThrottle wyświetlana; wybór serwera do połączenia.

### Etap 6 — Protokół WiThrottle: parser + zdarzenia
- `withrottle/parser.rs`: parser komunikatów przychodzących → `enum Event`
  (odpowiedniki wszystkich callbacków `MyDelegate`).
- `withrottle/protocol.rs`: konstrukcja komend wychodzących (multi-throttle,
  loco, speed, direction, turnout, route, e-stop, heartbeat, raw command).
- `withrottle/events.rs`: definicja `Event` + kanał do warstwy domeny/UI.
- Testy jednostkowe parsera na próbkach protokołu (host, bez sprzętu).
- **DoD:** parser przechodzi testy na przykładowych ramkach protokołu.

### Etap 7 — Klient TCP + pętla protokołu
- `net/tcp.rs` + `withrottle/client.rs`: task async — połączenie TCP,
  czytanie/pisanie, `requireHeartbeat`, watchdog braku odpowiedzi + reconnect.
- Podłączenie parsera (in) i buildera komend (out) do socketu.
- **DoD:** połączenie z realnym serwerem (JMRI/DCC-EX), heartbeat, odbiór wersji/rostera.

### Etap 8 — Model domenowy i logika sterowania
- `domain/model.rs`: `Throttle` × ≤6, `Consist`/`Loco`, roster, turnouts, routes,
  funkcje (32) + etykiety, prędkość/kierunek, multiplier prędkości.
- `domain/state.rs`: współdzielony stan (`Mutex`/`Signal`), aktualizowany zdarzeniami.
- Logika akcji: acquire/release/steal, speed up/down/stop/e-stop, direction,
  next throttle, power on/off, turnout throw/close, route set, funkcje (latching).
- **DoD:** sterowanie lokomotywą (prędkość/kierunek/funkcje) działa end-to-end.

### Etap 9 — Menu, ekrany i pełny UI
- `ui/screens.rs`: maszyna stanów menu (`*`…`#`), ekran throttle (jak w README),
  listy rostera/turnoutów/route/funkcji z paginacją, ekran hasła, komunikaty broadcast.
- Mapowanie klawiszy 0–9 na akcje domyślne/konfigurowalne (z configu).
- **DoD:** pełen zestaw ekranów i menu odwzorowany; nawigacja jak w oryginale.

### Etap 10 — Trwałość, bateria, uśpienie, i18n
- `storage/prefs.rs`: zapis/odczyt SSID, haseł, zapisanych loco (`sequential-storage`).
- `power/battery.rs`: pomiar napięcia (ADC), ikona/procent baterii.
- `power/sleep.rs`: deep sleep + wybudzanie przyciskiem, auto-shutdown po bezczynności.
- Uzupełnienie tłumaczeń (DE/IT/NL/CN) — opcjonalnie.
- **DoD:** funkcjonalny parytet z oryginałem WiTcontroller na docelowej płytce.

---

## 5. Mapowanie: C++ → Rust (skrót koncepcyjny)

| Oryginał (C++/Arduino) | Odpowiednik (Rust/embassy) |
|------------------------|----------------------------|
| `setup()` + `loop()` | `#[esp_hal::main]` init + spawn tasków, każdy task ma własną pętlę |
| Globalny stan `extern` (`WiTcontroller.h`) | `domain/state.rs` w `Mutex`/`Signal` (`embassy-sync`) |
| `MyDelegate` (callbacki) | `enum Event` + `Channel<Event>` |
| `wiThrottleProtocol.*` | moduł `withrottle/` (protocol/parser/client) |
| `String` (Arduino) | `heapless::String<N>` (bez heap) |
| tablice `[maxRoster]` itd. | `heapless::Vec<_, N>` |
| `Keypad`, `AiEsp32RotaryEncoder` | własne taski `input/*` z GPIO |
| `U8g2` | `ssd1306` + `embedded-graphics` |
| `Preferences` (NVS) | `storage/prefs.rs` (`sequential-storage`) |
| `#define` z config_* | `config/` (const/compile-time features) |
| `delay()`, `millis()` | `embassy_time::{Timer, Instant}` |

---

## 6. Ryzyka / punkty do zbadania

- **WiFi 6 w `esp-wifi`**: sprawdzić, na ile crate eksponuje funkcje HE/TWT/OFDMA na C6
  (część negocjowana sprzętowo). Zysk latency może wymagać ustawień QoS/power-save.
- **mDNS w no-std**: `edge-mdns` vs własny minimalny klient zapytań.
- **Brak `String`/heap**: dobrać rozmiary `heapless` (SSID, hasła, nazwy loco, komunikaty).
- **Enkoder na C6**: użycie peryferium PCNT (jeśli dostępne w `esp-hal`) vs GPIO+interrupt.
- **Piny ESP32-C6 ≠ klasyczny ESP32**: mapowanie pinów to osobne zadanie (etap 11).
- **Fonty/i18n CJK**: chińskie znaki wymagają większych fontów — priorytet niski.

---

## 7. Weryfikacja / testy

- Testy jednostkowe **parsera protokołu** i **buildera komend** (host, `cargo test`
  w osobnym crate/`no_std`-friendly) — bez sprzętu.
- Testy integracyjne na sprzęcie po etapach 4, 7, 8 (WiFi, protokół, sterowanie).
- Log `defmt`/serial jako główne narzędzie diagnostyczne.

---

## 8. Kolejne zadania (poza tym planem — do zrobienia później)

1. **Dostosowanie do konkretnej płytki ESP32-C6** — mapowanie pinów, wybór peryferiów,
   ew. inny wyświetlacz/enkoder.
2. **Optymalizacje WiFi 6 / latency** — TWT/power-save, QoS, tuning `embassy-net`/smoltcp,
   minimalizacja RTT komend WiThrottle, batchowanie.
3. **Rozwój funkcjonalności** — ponad parytet z oryginałem (nowe akcje, lepsze UI,
   auto-reconnect, itp.).

---

## 9. Następny krok

Po akceptacji planu proponuję zacząć od **Etapu 0** (bootstrap projektu Cargo dla
ESP32-C6 + smoke test), a następnie **Etapu 6** (parser protokołu WiThrottle z testami),
bo protokół jest sercem aplikacji i można go rozwijać/testować niezależnie od sprzętu.
