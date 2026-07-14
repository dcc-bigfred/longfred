//! Układ klawiatury/enkodera i parametry (timingi, czułość).
//! Piny pochodzą z `config::board` (BSP) — tu ich NIE definiujemy.

use crate::config::board;

pub const ROWS: usize = board::KEYPAD_ROW_PINS.len();
pub const COLS: usize = board::KEYPAD_COL_PINS.len();

/// Układ klawiszy matrycy 4x3 (jak KEYPAD_KEYS w oryginale).
pub const KEYMAP: [[char; COLS]; ROWS] = [
    ['1', '2', '3'],
    ['4', '5', '6'],
    ['7', '8', '9'],
    ['*', '0', '#'],
];

pub const KEYPAD_DEBOUNCE_MS: u64 = 10;
pub const KEYPAD_HOLD_MS: u64 = 200;
pub const ROTARY_ENCODER_STEPS: u8 = 2;
pub const ENCODER_SENSITIVITY: u8 = 85;
pub const EC11_PULLUPS_REQUIRED: bool = false;
