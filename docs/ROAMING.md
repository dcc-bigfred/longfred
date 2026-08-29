# Wi-Fi Roaming — LongFred

Szybkie (<1 s) przełączanie między AP dla sterowników LongFred na ESP32-C6,
z konfiguracją infrastruktury TP-Link Omada (OC200 + EAP610/613/650 + TL-SF1006P).

## 1. Cel i zakres

LongFred to bezprzewodowy pilot DCC na ESP32-C6 (WiFi 6, 2.4 GHz).
Na dużej makietce z wieloma sterownikami pilot potrzebuje płynnego
przełączania między punktami dostępowymi (AP), bez utraty sterowania
i z czasem recovery poniżej sekundy.

Niniejszy dokument opisuje:

- wymaganą konfigurację infrastruktury Omada,
- parametry `RadioConfig` w firmware (14 pól),
- mechanizm przypinania IPv4,
- status wsparcia 802.11k/v/r w esp-radio (upstream),
- edycję ustawień (OLED, HTTP, reset),
- rozwiązywanie problemów,
- mierzenie czasu roamu.

Grupa docelowa: operatorzy makietek i konfiguratorzy sieci Omada.

## 2. Warstwa infrastruktury

### Wymagana konfiguracja Omada

| Ustawienie | Wartość | Uwagi |
|---|---|---|
| SSID | jeden, wspólny dla wszystkich EAP | Inaczej przypinanie IP i 802.11r nie mają sensu |
| VLAN | jeden, wspólny dla wszystkich EAP | Ten sam L2 na TL-SF1006P |
| 802.11r (FT-PSK) | włączone, over-the-air | Profil WLAN w Omada |
| 802.11k (RRM) | włączone | Profil WLAN w Omada |
| 802.11v (BTM) | włączone | Profil WLAN w Omada |
| Band steering | wyłączone | ESP32-C6 to tylko 2.4 GHz |
| TX power | -12 do -15 dBm na granicy | Strefy pokrycia muszą się mocno nakładać |
| Rezerwacje DHCP | per MAC, dla wszystkich sterowników | Usuwa ryzyko konfliktu IP przy przypinaniu |

### Topologia

```
Internet
  |
OC200 (kontroler Omada)
  |
TL-SF1006P (switch PoE)
  |
  +--+--+--+--
  |  |  |  |
EAP613 EAP613 EAP650 EAP610
```

Wszystkie EAP na tym samym L2 (TL-SF1006P bez routingowania między nimi).

## 3. Parametry `RadioConfig`

| Parametr | Zakres | Domyślnie | Opis |
|---|---|---|---|
| `roam_enabled` | bool | false | Master switch. false = sticky client (trzyma AP do utraty sygnału). true = `RoamEngine` podejmuje decyzje na podstawie RSSI |
| `rrm_enabled` | bool | true | 802.11k (RRM). No-op do wdrożenia Tier B w esp-radio |
| `btm_enabled` | bool | true | 802.11v (BTM). No-op do wdrożenia Tier B w esp-radio |
| `ft_enabled` | bool | true | 802.11r (FT). No-op do wdrożenia Tier C w esp-radio |
| `power_save_off` | bool | true | Wyłącza TWT/modem-sleep (latency > energy) |
| `enable_11ax` | bool | true | 802.11ax (OFDMA) na 2.4 GHz |
| `roam_rssi_threshold` | -90..=-50 | -72 | Próg w -dBm, poniżej którego `RoamEngine` szuka lepszego AP |
| `roam_hysteresis_db` | 3..=20 | 8 | Minimalna różnica RSSI (dB) między aktualnym a kandydatem, by roam się odbył. Zabezpieczenie przed ping-pong |
| `roam_debounce_samples` | 1..=10 | 3 | Ile kolejnych próbek poniżej progu zanim `RoamEngine` zareaguje. Zabezpieczenie przed chwilowymi spadkami |
| `roam_scan_interval_s` | 1..=60 | 10 | Minimalny odstęp (s) między skanami inicjowanymi przez roaming |
| `roam_sample_ms` | 100..=2000 | 250 | Częstotliwość próbkowania RSSI (ms) |
| `ip_pinning` | bool | true | Przypinanie IPv4 po zerwaniu linku (patrz sekcja 4) |
| `ip_pin_max_gap_s` | 5..=3600 | 120 | Po tylu sekundach przerwy odpinamy adres |
| `dhcp_discover_timeout_s` | 1..=30 | 2 | Timeout DISCOVER w smoltcp (domyślnie 10) |

## 4. Przypinanie IPv4

### Mechanizm

Po zerwaniu linku (roam, utrata zasięgu, restart AP) firmware przypina poprzedni adres
jako statyczny, zamiast od nowa odpytywać DHCP. Po powrocie linku adres jest od razu gotowy — zero sekund na DORA.

### Walidacja

- Jeśli SSID różni się od zapamiętanego — odpiń od razu.
- W przeciwnym razie jeden ICMP echo do bramy z krótkim timeoutem.
  Niepowodzenie oznacza inną podsieć lub VLAN — odpiń i pozwól na normalne DORA.

### Watchdog

Po `ip_pin_max_gap_s` sekundach nieobecności odpinamy, by uniknąć powrotu do zupełnie innej sieci po długiej nieobecności.

### Dlaczego rezerwacje DHCP per MAC są zalecane

Przypięty adres nie jest odnawiany przez całą sesję. Przy krótkiej dzierżawie serwer może go komuś przydzielić.
Rezerwacja usuwa ten problem u źródła: przypięty adres jest z definicji tym, który serwer i tak by przydzielił.

### Dlaczego nie odświeżamy dzierżawy w tle

Powrót do DHCP zabija otwarte sesje TCP — adres znika natychmiast, zanim DORA się zakończy.
Higiena dzierżawy jest przeniesiona na rezerwacje DHCP.

### Ograniczenia

- Reboot i deep sleep czyszczą `LAST_LEASE` (RAM) — po wybudzeniu jest normalne DORA.
- Statyczne IP z NVS czyni mechanizm bezprzedmiotowym — bez socketu DHCP nie ma czego przypinać.

## 5. Status wsparcia 802.11k/v/r w esp-radio

| Tier | Co | Status |
|---|---|---|
| A | BSSID lock + skan sąsiadów | Działa dzisiaj, bez zmian w bibliotekach |
| B | 802.11k/v (RRM/BTM) | Wymaga PR do esp-radio + esp-wifi-sys. Do wdrożenia przez fork z `[patch.crates-io]` |
| C | 802.11r (FT) | Wymaga dodatkowo przebudowy blobów supplikanta. Wymaga pomiaru budżetu flash/RAM |

Do czasu wdrożenia Tier B/C flagi `rrm_enabled`/`btm_enabled`/`ft_enabled` są no-op (domyślnie `true`, ale esp-radio je ignoruje). `roam_enabled` działa zawsze (Tier A).

## 6. Edycja ustawień

### OLED

Wi-Fi settings → Ustawienia radia → wybór pola → edycja:

- pole bool: lista `1: Wł.` / `2: Wył.`
- pole liczbowe: klawiatura cyfrowa, clamp do zakresu przy commicie
- OK → komunikat "Radio zapisane" → powrót do listy

### HTTP w trybie programowania

`http://192.168.4.1/` → sekcja "Radio" → Save → Exit (reboot).

### Reset do ustawień fabrycznych

Przez Soft-AP — usunięcie credentials czyści też `RadioConfig` do default.

## 7. Rozwiązywanie problemów

| Symptom | Przyczyna | Rozwiązanie |
|---|---|---|
| Sterownik nie roamuje mimo słabego RSSI | `roam_enabled` wyłączone, `roam_rssi_threshold` za niski, Tier B/C nie wdrożony | Włącz `roam_enabled`, podnieś próg, wdróż Tier B/C |
| Roam działa, ale sesja TCP pada na długo | `ip_pinning` wyłączone, brak rezerwacji DHCP, `dhcp_discover_timeout_s` za wysoki | Włącz `ip_pinning`, dodaj rezerwacje, obniż timeout |
| Ping-pong między AP | `roam_hysteresis_db` i `roam_debounce_samples` za niskie | Zwiększ wartości |
| Sterownik gubi adres po długiej przerwie | Zamierzone (watchdog `ip_pin_max_gap_s`) | Zwiększ parametr lub wyłącz przypinanie |
| Po roamie inny adres IP niż przed | Brak rezerwacji DHCP per MAC, albo inny VLAN na AP | Sprawdź konfigurację Omady |

## 8. Mierzenie czasu roamu

### Logi `wifi`

```
wifi connected              BSSID=aa:bb:cc:dd:ee:01 channel=6
wifi roam: rssi=-78 -> scan
wifi roam: target BSSID=aa:bb:cc:dd:ee:02 channel=11
wifi connected              BSSID=aa:bb:cc:dd:ee:02 channel=11
net ready: ip=192.168.1.42
```

Czas od `rssi=-78` do `net ready` to czas roamu + recovery.

### Diagnostics screen

RSSI chart (próbkowanie co `roam_sample_ms`), ping RTT do stacji komend.
