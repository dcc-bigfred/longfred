#![no_std]
//! LongFred firmware: biblioteka aplikacji (konfiguracja, domena, wejście, UI, sieć).
//! Entry-point i inicjalizacja HAL są w `src/bin/main.rs`.

pub mod config;
pub mod domain;
pub mod input;
pub mod power;
pub mod storage;
