//! Two-button hold chord detector (Shift1+Stop → programming mode).

pub use longfred_proto::input_map::ChordState as ChordDetector;

pub const PROGRAMMING_CHORD_MS: u64 = 8_000;
