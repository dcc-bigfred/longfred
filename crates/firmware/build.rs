//! Build script for LongFred firmware (linker args and friendly link errors).

fn main() {
    linker_be_nice();
    println!("cargo:rustc-link-arg=-Tlinkall.x");
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
