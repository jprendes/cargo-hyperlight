use std::env;
use std::env::consts::ARCH;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use const_format::formatcp;

use crate::cargo::{CargoCmd as _, cargo};

pub struct Args {
    pub manifest_path: Option<PathBuf>,
    pub target_dir: PathBuf,
    pub target: String,
    pub cargo_args: Vec<OsString>,
}

pub enum Command {
    Build(Args),
    Clippy(Args),
}

impl Command {
    pub fn parse() -> Result<Command> {
        let args = env::args_os().enumerate().filter_map(|(i, arg)| {
            if i == 1 && arg == "hyperlight" {
                None
            } else {
                Some(arg)
            }
        });

        let cli = CliImpl::parse_from(args);
        cli.command.try_into()
    }
}

impl TryFrom<ArgsImpl> for Args {
    type Error = anyhow::Error;

    fn try_from(value: ArgsImpl) -> Result<Self> {
        let manifest_path = value.manifest_path;
        let target_dir = match value.target_dir {
            Some(dir) => dir,
            None => resolve_target_dir(&manifest_path)?,
        };
        let cwd = env::current_dir().context("Failed to get current directory")?;
        let target_dir = cwd.join(target_dir);

        Ok(Args {
            manifest_path,
            target_dir,
            target: value.target,
            cargo_args: value.cargo_args,
        })
    }
}

impl TryFrom<CommandImpl> for Command {
    type Error = anyhow::Error;
    fn try_from(value: CommandImpl) -> Result<Self> {
        match value {
            CommandImpl::Build(args) => Ok(Command::Build(args.try_into()?)),
            CommandImpl::Clippy(args) => Ok(Command::Clippy(args.try_into()?)),
        }
    }
}

const DEFAULT_TARGET: &str = const { formatcp!("{ARCH}-hyperlight-none") };

#[derive(Parser)]
#[command(version, about, trailing_var_arg = true)]
#[command(propagate_version = true)]
struct CliImpl {
    #[command(subcommand)]
    command: CommandImpl,
}

#[derive(Subcommand)]
enum CommandImpl {
    /// Build a hyperlight guest binary
    Build(ArgsImpl),

    /// Run clippy on a hyperlight guest binary
    Clippy(ArgsImpl),
}

#[derive(Parser)]
struct ArgsImpl {
    /// Path to Cargo.toml
    #[arg(long, value_name = "PATH")]
    manifest_path: Option<PathBuf>,

    /// Directory for all generated artifacts
    #[arg(long, value_name = "DIRECTORY")]
    target_dir: Option<PathBuf>,

    /// Target triple to build for
    #[arg(long, value_name = "TRIPLE", default_value = DEFAULT_TARGET)]
    target: String,

    /// Arguments to pass to cargo
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    cargo_args: Vec<OsString>,
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

fn resolve_target_dir(manifest_path: &Option<PathBuf>) -> Result<PathBuf> {
    let output = cargo("metadata")
        .manifest_path(manifest_path)
        .arg("--format-version=1")
        .arg("--no-deps")
        .output()
        .context("Failed to get cargo metadata")?;

    ensure!(output.status.success(), "Failed to get cargo metadata");

    let metadata: CargoMetadata =
        serde_json::from_slice(&output.stdout).context("Failed to parse cargo metadata")?;

    Ok(metadata.target_directory)
}
