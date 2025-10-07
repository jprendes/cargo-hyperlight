use std::collections::HashMap;
use std::ffi::OsStr;
use std::iter;

use anyhow::Result;

mod cargo_cmd;
mod cli;
mod command;
mod sysroot;
mod toolchain;

use cargo_cmd::CargoCmd;
use cli::Args;
pub use command::Command;

/// Constructs a new `Command` for launching cargo targeting
/// [hyperlight](https://github.com/hyperlight-dev/hyperlight) guest code.
///
/// The value of the `CARGO` environment variable is used if it is set; otherwise, the
/// default `cargo` from the system PATH is used.
/// If `RUSTUP_TOOLCHAIN` is set in the environment, it is also propagated to the
/// child process to ensure correct functioning of the rustup wrappers.
///
/// The default configuration is:
/// - No arguments to the program
/// - Inherits the current process's environment
/// - Inherits the current process's working directory
///
/// # Errors
///
/// This function will return an error if:
/// - If the `CARGO` environment variable is set but it specifies an invalid path
/// - If the `CARGO` environment variable is not set and the `cargo` program cannot be found in the system PATH
///
/// # Examples
///
/// Basic usage:
///
/// ```rust
/// use cargo_hyperlight::cargo;
///
/// let command = cargo().unwrap();
/// ```
pub fn cargo() -> Result<Command> {
    Command::new()
}

impl Args {
    pub fn sysroot_dir(&self) -> std::path::PathBuf {
        self.target_dir.join("sysroot")
    }

    pub fn triplet_dir(&self) -> std::path::PathBuf {
        self.sysroot_dir()
            .join("lib")
            .join("rustlib")
            .join(&self.target)
    }

    pub fn build_dir(&self) -> std::path::PathBuf {
        self.sysroot_dir().join("target")
    }

    pub fn libs_dir(&self) -> std::path::PathBuf {
        self.triplet_dir().join("lib")
    }

    pub fn includes_dir(&self) -> std::path::PathBuf {
        self.triplet_dir().join("include")
    }

    pub fn crate_dir(&self) -> std::path::PathBuf {
        self.sysroot_dir().join("crate")
    }

    pub fn build_plan_dir(&self) -> std::path::PathBuf {
        self.sysroot_dir().join("build-plan")
    }
}

trait CargoCommandExt {
    fn prepare_sysroot(
        &mut self,
        envs: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
    ) -> Result<&mut Self>;
}

impl CargoCommandExt for std::process::Command {
    fn prepare_sysroot(
        &mut self,
        envs: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
    ) -> Result<&mut Self> {
        // skip the cargo subcommand
        let args = self.get_args().skip(1);

        // but append a fake binary name so that clap can parse the arguments
        let args = iter::once(OsStr::new("cargo-hyperlight")).chain(args);

        // get the current environment variables and merge them with the command's env
        let envs = envs.into_iter().collect::<Vec<_>>();
        let envs = envs
            .iter()
            .map(|(k, v)| (k.as_ref(), Some(v.as_ref())))
            .chain(self.get_envs())
            .collect::<HashMap<_, _>>()
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)));

        // parse the arguments and environment variables
        let args = Args::parse_from(args, envs, self.get_current_dir())?;

        // Build sysroot
        let sysroot = sysroot::build(&args)?;

        // Build toolchain
        toolchain::prepare(&args)?;

        let triplet = &args.target;

        let cc_bin = toolchain::find_cc()?;
        let ar_bin = toolchain::find_ar()?;

        // populate the command with the necessary environment variables
        self.target(triplet)
            .sysroot(&sysroot)
            .cc_env(triplet, &cc_bin)
            .ar_env(triplet, &ar_bin)
            .append_cflags(triplet, toolchain::cflags(&args));

        Ok(self)
    }
}
