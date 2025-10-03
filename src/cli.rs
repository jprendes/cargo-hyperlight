use std::collections::HashMap;
use std::env;
use std::env::consts::ARCH;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use const_format::formatcp;

use crate::cargo::{CargoCmd as _, cargo};

pub struct Args {
    pub manifest_path: Option<PathBuf>,
    pub target_dir: PathBuf,
    pub target: String,
    pub env: HashMap<OsString, OsString>,
}

impl Args {
    pub fn parse_from(
        args: impl IntoIterator<Item = impl Into<OsString> + Clone>,
        env: impl IntoIterator<Item = (impl Into<OsString>, impl Into<OsString>)>,
    ) -> Result<Args> {
        let mut args = ArgsImpl::parse_from(args);
        args.env = env.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        args.try_into()
    }
}

impl TryFrom<ArgsImpl> for Args {
    type Error = anyhow::Error;

    fn try_from(value: ArgsImpl) -> Result<Self> {
        let manifest_path = value.manifest_path;

        let target_dir = match value.target_dir {
            Some(dir) => dir,
            None => resolve_target_dir(&manifest_path, &value.env)?,
        };

        let target = match value.target {
            Some(triplet) => triplet,
            None => resolve_target(&value.env)?,
        };

        let cwd = env::current_dir().context("Failed to get current directory")?;
        let target_dir = cwd.join(target_dir);

        Ok(Args {
            manifest_path,
            target_dir,
            target,
            env: value.env,
        })
    }
}

const DEFAULT_TARGET: &str = const { formatcp!("{ARCH}-hyperlight-none") };

#[derive(Parser)]
#[command(disable_help_subcommand = true)]
struct ArgsImpl {
    /// Path to Cargo.toml
    #[arg(long, value_name = "PATH")]
    manifest_path: Option<PathBuf>,

    /// Directory for all generated artifacts
    #[arg(long, value_name = "DIRECTORY")]
    target_dir: Option<PathBuf>,

    /// Target triple to build for
    #[arg(long, value_name = "TRIPLE")]
    target: Option<String>,

    /// Arguments to pass to cargo
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    cargo_args: Vec<OsString>,

    #[arg(skip)]
    env: HashMap<OsString, OsString>,
}

#[derive(Subcommand)]
enum BuildCommands {
    /// does testing things
    Test {
        /// lists test values
        #[arg(short, long)]
        list: bool,
    },
}

#[derive(serde::Deserialize)]
struct CargoMetadata {
    target_directory: PathBuf,
}

fn resolve_target_dir(
    manifest_path: &Option<PathBuf>,
    env: &HashMap<OsString, OsString>,
) -> Result<PathBuf> {
    let output = cargo()
        .env_clear()
        .envs(env.iter())
        .arg("metadata")
        .manifest_path(manifest_path)
        .arg("--format-version=1")
        .arg("--no-deps")
        .checked_output()
        .context("Failed to get cargo metadata")?;

    let metadata: CargoMetadata =
        serde_json::from_slice(&output.stdout).context("Failed to parse cargo metadata")?;

    Ok(metadata.target_directory)
}

fn resolve_target(env: &HashMap<OsString, OsString>) -> Result<String> {
    let output = cargo()
        .env_clear()
        .envs(env.iter())
        .arg("config")
        .arg("get")
        .arg("--quiet")
        .arg("--format=json-value")
        .arg("-Zunstable-options")
        .arg("build.target")
        // cargo config is an unstable feature
        .allow_unstable()
        // use output instead of checked_output
        // as cargo will error if build.target is not set
        .output()
        .context("Failed to get cargo config")?;

    let target = String::from_utf8_lossy(&output.stdout);
    let target = target.trim();
    let target = target.trim_matches(|c| c == '"' || c == '\'');

    if target.is_empty() {
        Ok(DEFAULT_TARGET.into())
    } else {
        Ok(target.into())
    }
}
