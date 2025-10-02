use std::collections::HashMap;
use std::ffi::OsStr;
use std::{env, iter};

use anyhow::Result;

mod cargo;
mod cli;
mod sysroot;
mod toolchain;

use cargo::CargoCmd;
pub use cargo::cargo;
use cli::Args;

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

pub trait CargoCommandExt {
    fn prepare_sysroot(&mut self) -> Result<&mut Self>;
}

impl CargoCommandExt for std::process::Command {
    fn prepare_sysroot(&mut self) -> Result<&mut Self> {
        // skip the cargo subcommand
        let args = self.get_args().skip(1);

        // but append a fake binary name so that clap can parse the arguments
        let args = iter::once(OsStr::new("cargo-hyperlight")).chain(args);

        // get the current environment variables and merge them with the command's env
        let os_env = env::vars_os().collect::<Vec<_>>();
        let envs = os_env
            .iter()
            .map(|(k, v)| (k.as_os_str(), Some(v.as_os_str())))
            .chain(self.get_envs())
            .collect::<HashMap<_, _>>()
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)));

        // parse the arguments and environment variables
        let args = Args::parse_from(args, envs)?;

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
