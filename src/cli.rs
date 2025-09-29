use std::env;
use std::env::consts::ARCH;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use const_format::formatcp;
use toml_edit::DocumentMut;

pub struct Args {
    pub command: &'static str,
    pub manifest_path: PathBuf,
    pub target_dir: PathBuf,
    pub target: String,
    pub cargo_args: Vec<OsString>,
}

impl Args {
    pub fn parse() -> Args {
        let args = env::args_os().enumerate().filter_map(|(i, arg)| {
            if i == 1 && arg == "hyperlight" {
                None
            } else {
                Some(arg)
            }
        });

        let args = ArgsImpl::parse_from(args);
        let (command, args) = match args.command {
            Command::Build(args) => ("build", args),
            Command::Clippy(args) => ("clippy", args),
        };

        let manifest_path = args
            .manifest_path
            .or_else(|| find_cargo_toml())
            .expect("Error: Could not find Cargo.toml");
        let target_dir = args
            .target_dir
            .unwrap_or_else(|| resolve_target_dir(&manifest_path));

        let cwd = env::current_dir().expect("Failed to get current directory");
        let manifest_path = cwd.join(manifest_path);
        let target_dir = cwd.join(target_dir);

        Args {
            command,
            manifest_path: manifest_path,
            target_dir: target_dir,
            target: args.target,
            cargo_args: args.cargo_args,
        }
    }
}

const DEFAULT_TARGET: &str = const { formatcp!("{ARCH}-hyperlight-none") };

#[derive(Parser)]
#[command(version, about, trailing_var_arg = true)]
#[command(propagate_version = true)]
struct ArgsImpl {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build a hyperlight guest binary
    Build(BuildCommand),

    /// Run clippy on a hyperlight guest binary
    Clippy(BuildCommand),
}

#[derive(Parser)]
struct BuildCommand {
    /// Path to Cargo.toml
    #[arg(long, value_name = "PATH", default_value = "Cargo.toml")]
    manifest_path: Option<PathBuf>,

    /// Directory for all generated artifacts
    #[arg(long, value_name = "DIRECTORY", default_value = "target")]
    target_dir: Option<PathBuf>,

    /// Target triple to build for
    #[arg(long, value_name = "TRIPLE", default_value = DEFAULT_TARGET)]
    target: String,

    /// Arguments to pass to cargo
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    cargo_args: Vec<OsString>,
}

#[derive(Subcommand)]
enum Commands {
    /// does testing things
    Test {
        /// lists test values
        #[arg(short, long)]
        list: bool,
    },
}

fn find_cargo_toml() -> Option<PathBuf> {
    let mut current = env::current_dir().ok()?;

    loop {
        let manifest = current.join("Cargo.toml");
        if manifest.exists() {
            return Some(manifest);
        }

        if !current.pop() {
            break;
        }
    }

    None
}

fn resolve_target_dir(manifest_path: &Path) -> PathBuf {
    // Check CARGO_BUILD_TARGET_DIR
    if let Some(dir) = env::var_os("CARGO_BUILD_TARGET_DIR") {
        return dir.into();
    }

    // Check CARGO_TARGET_DIR
    if let Some(dir) = env::var_os("CARGO_TARGET_DIR") {
        return dir.into();
    }

    // Check .cargo/config.toml
    if let Some(dir) = read_cargo_config_target_dir(manifest_path) {
        return dir.into();
    }

    "target".into()
}

fn read_cargo_config_target_dir(manifest_path: &Path) -> Option<PathBuf> {
    // Start from the directory containing Cargo.toml
    let mut current = manifest_path.parent()?.to_path_buf();

    loop {
        // Check both config.toml and config
        for config_name in &["config.toml", "config"] {
            let config_path = current.join(".cargo").join(config_name);
            if config_path.exists() {
                if let Ok(contents) = fs::read_to_string(&config_path) {
                    if let Some(target_dir) = parse_cargo_config(&contents) {
                        return Some(target_dir);
                    }
                }
            }
        }

        if !current.pop() {
            return None;
        }
    }
}

fn parse_cargo_config(contents: &str) -> Option<PathBuf> {
    let doc = contents.parse::<DocumentMut>().ok()?;

    doc.get("build")
        .and_then(|build| build.get("target-dir"))
        .and_then(|value| value.as_str())
        .map(|s| s.into())
}
