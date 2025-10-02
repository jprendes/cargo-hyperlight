use std::env;

use anyhow::{Context, Result, ensure};
use cargo_hyperlight::{CargoCommandExt as _, cargo};

fn main() -> Result<()> {
    let args = env::args_os().enumerate().filter_map(|(i, arg)| {
        // skip the binary name and the "hyperlight" subcommand if present
        if i == 0 || (i == 1 && arg == "hyperlight") {
            None
        } else {
            Some(arg)
        }
    });

    let status = cargo()
        .args(args)
        .prepare_sysroot()
        .context("Failed to prepare sysroot")?
        .status()
        .context("Failed to execute cargo")?;

    ensure!(status.success(), "Cargo command failed");

    Ok(())
}
