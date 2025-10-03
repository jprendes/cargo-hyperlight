use std::env;

use anyhow::Result;
use cargo_hyperlight::CargoCommand;

fn main() -> Result<()> {
    let args = env::args_os().enumerate().filter_map(|(i, arg)| {
        // skip the binary name and the "hyperlight" subcommand if present
        if i == 0 || (i == 1 && arg == "hyperlight") {
            None
        } else {
            Some(arg)
        }
    });

    CargoCommand::new().args(args).exec()
}
