//! Board Support Package: centralny, jedyny pinout płytki.
//!
//! WSZYSTKIE fizyczne numery GPIO są tutaj. Zmiana płytki / dostrojenie do
//! ESP32-C6 (Etap 11) = edycja tylko tego pliku. Nigdzie indziej w kodzie nie
//! wpisujemy surowych numerów pinów — kod odwołuje się do `board::*`.
//!
//! UWAGA: obecne wartości to placeholdery (klasyczny ESP32).

/// Numer pinu GPIO (surowy indeks na obudowie).
pub type Gpio = u8;

// --- Klawiatura matrycowa 4x3 ---
// TODO(etap-11): dopasować do ESP32-C6.
pub const KEYPAD_ROW_PINS: [Gpio; 4] = [19, 18, 17, 16];
pub const KEYPAD_COL_PINS: [Gpio; 3] = [4, 0, 2];

// --- Enkoder obrotowy (KY-040 / EC11) ---
pub const ENCODER_A: Gpio = 12;
pub const ENCODER_B: Gpio = 14;
pub const ENCODER_BUTTON: Gpio = 13;

// --- Wyświetlacz OLED (I2C) ---
pub const OLED_SDA: Gpio = 23;
pub const OLED_SCL: Gpio = 22;
pub const OLED_I2C_FREQ_KHZ: u32 = 400;
/// Adres I2C wyświetlacza (0x3C = standard SSD1306).
pub const OLED_I2C_ADDRESS: u8 = 0x3C;

// --- Pomiar baterii (ADC) ---
pub const BATTERY_ADC: Gpio = 34;
