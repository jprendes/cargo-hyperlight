use anyhow::{Context, Result, ensure};

mod cargo;
mod cli;
mod sysroot;
mod toolchain;

use cargo::{CargoCmd, cargo};
pub use cli::{Args, Command};

impl Command {
    pub fn run(&self) -> Result<()> {
        let (command, args) = match self {
            cli::Command::Build(args) => ("build", args),
            cli::Command::Clippy(args) => ("clippy", args),
        };

        // Build sysroot
        let sysroot = sysroot::build(args)?;

        // Build toolchain
        let toolchain = toolchain::prepare(args)?;

        let triplet = &args.target;

        let cc_bin = toolchain::find_cc(&toolchain)?;
        let ar_bin = toolchain::find_ar()?;

        // Execute cargo
        let status = cargo(command)
            .target(triplet)
            .target_dir(&args.target_dir)
            .manifest_path(&args.manifest_path)
            // Add remaining arguments
            .args(&args.cargo_args)
            // Populate rustflags with sysroot and codegen options
            .env("RUSTFLAGS", sysroot::rustflags(&sysroot))
            // Add the toolchain to PATH
            .env("PATH", toolchain::path_with(&toolchain))
            // Set the hyperlight toolchain environment variables
            //.env("HYPERLIGHT_GUEST_TOOLCHAIN_ROOT", &toolchain)
            .cc_env(triplet, &cc_bin)
            .ar_env(triplet, &ar_bin)
            .cflags_env(triplet, toolchain::cflags(triplet))
            .status()
            .context("Failed to execute cargo")?;

        // Check exit status
        ensure!(
            status.success(),
            "Cargo exited with non-zero status: {}",
            status
        );

        Ok(())
    }
}
