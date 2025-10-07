use std::env;

use cargo_hyperlight::cargo;

fn main() -> ! {
    let args = env::args_os().enumerate().filter_map(|(i, arg)| {
        // skip the binary name and the "hyperlight" subcommand if present
        if i == 0 || (i == 1 && arg == "hyperlight") {
            None
        } else {
            Some(arg)
        }
    });

    cargo()
        .expect("Failed to create cargo command")
        .args(args)
        .exec()
}
