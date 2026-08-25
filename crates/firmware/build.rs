//! Build script for LongFred firmware (linker args and friendly link errors).

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    linker_be_nice();
    emit_battery_factor();
    println!("cargo:rustc-link-arg=-Tlinkall.x");
}

fn emit_battery_factor() {
    println!("cargo:rerun-if-env-changed=LONGFRED_BATTERY_FACTOR");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let dest = Path::new(&out_dir).join("battery_factor.rs");
    let contents = match env::var("LONGFRED_BATTERY_FACTOR") {
        Ok(raw) if !raw.is_empty() => {
            let value: f32 = raw.parse().unwrap_or_else(|_| {
                panic!("LONGFRED_BATTERY_FACTOR must be a finite f32, got {raw:?}")
            });
            if !value.is_finite() || !(0.5..=10.0).contains(&value) {
                panic!(
                    "LONGFRED_BATTERY_FACTOR must be in 0.5..=10.0, got {value}"
                );
            }
            format!(
                "/// ADC-to-voltage scaling factor (build-time override).\n\
                 pub const BATTERY_CONVERSION_FACTOR: f32 = {value}_f32;\n"
            )
        }
        _ => {
            "/// ADC-to-voltage scaling factor (hardware calibration from the pin map).\n\
             ///\n\
             /// Override at build time with `LONGFRED_BATTERY_FACTOR` / Makefile `BATTERY_FACTOR`.\n\
             pub const BATTERY_CONVERSION_FACTOR: f32 = crate::board::pins::BATTERY_CONVERSION_FACTOR;\n"
                .into()
        }
    };
    fs::write(dest, contents).expect("write battery_factor.rs");
}

// Build script is a host tool, not runtime code; the workspace `unwrap_used`
// deny policy targets firmware/runtime paths. Allow here for the one-shot
// linker configuration.
#[allow(clippy::unwrap_used)]
fn linker_be_nice() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let kind = &args[1];
        let what = &args[2];

        match kind.as_str() {
            "undefined-symbol" => match what.as_str() {
                what if what.starts_with("_defmt_") => {
                    eprintln!();
                    eprintln!(
                        "defmt not found - make sure defmt.x is added and use defmt_rtt as _"
                    );
                    eprintln!();
                }
                "_stack_start" => {
                    eprintln!();
                    eprintln!("Is the linker script linkall.x missing?");
                    eprintln!();
                }
                what if what.starts_with("esp_rtos_") => {
                    eprintln!();
                    eprintln!(
                        "esp-radio has no scheduler - initialize esp-rtos or provide external scheduler"
                    );
                    eprintln!();
                }
                _ => (),
            },
            _ => std::process::exit(1),
        }

        std::process::exit(0);
    }

    println!(
        "cargo:rustc-link-arg=--error-handling-script={}",
        std::env::current_exe().unwrap().display()
    );
}
