use std::ffi::OsString;
use std::ops::Not as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use target_spec_json::TargetSpec;

use crate::cargo::{CargoCmd, cargo};
use crate::cli::Args;

const CARGO_TOML: &str = include_str!("dummy/_Cargo.toml");
const LIB_RS: &str = include_str!("dummy/_lib.rs");

#[derive(serde::Deserialize)]
struct CargoCheck {
    filenames: Vec<PathBuf>,
}

pub fn build(args: &Args) -> Result<PathBuf> {
    let target_spec = match args.target.as_str() {
        "x86_64-hyperlight-none" => {
            let mut spec = get_spec(args, "x86_64-unknown-none")?;
            spec.entry_name = Some("entrypoint".into());
            spec.code_model = Some("small".into());
            spec.linker = Some("rust-lld".into());
            spec.linker_flavor = Some("gnu-lld".into());
            spec.pre_link_args =
                Some([("gnu-lld".to_string(), vec!["-znostart-stop-gc".to_string()])].into());
            spec
        }
        triplet => bail!("Unsupported target triple: {triplet}"),
    };

    let sysroot_dir = args.target_dir.join("sysroot");
    let target_dir = sysroot_dir.join("target");
    let triplet_dir = sysroot_dir.join("lib").join("rustlib").join(&args.target);
    let crate_dir = sysroot_dir.join("crate");

    std::fs::create_dir_all(&triplet_dir).context("Failed to create sysroot directories")?;
    std::fs::write(
        triplet_dir.join("target.json"),
        serde_json::to_string_pretty(&target_spec).unwrap(),
    )
    .context("Failed to write target spec file")?;

    std::fs::create_dir_all(&crate_dir).context("Failed to create target directory")?;
    std::fs::write(crate_dir.join("Cargo.toml"), CARGO_TOML)
        .context("Failed to write Cargo.toml")?;
    std::fs::write(crate_dir.join("lib.rs"), LIB_RS).context("Failed to write lib.rs")?;

    // if we are using rustup, ensure that the rust-src component is installed
    if let Some(rustup_toolchain) = std::env::var_os("RUSTUP_TOOLCHAIN") {
        let _ = std::process::Command::new("rustup")
            .arg("component")
            .arg("add")
            .arg("rust-src")
            .arg("--toolchain")
            .arg(rustup_toolchain)
            .status();
    }

    // Build the sysroot
    let success = cargo("rustc")
        .target(&args.target)
        .manifest_path(&Some(crate_dir.join("Cargo.toml")))
        .target_dir(&target_dir)
        .arg("-Zbuild-std=core,alloc")
        .arg("-Zbuild-std-features=compiler_builtins/mem")
        .arg("--release")
        .env("RUSTC_BOOTSTRAP", "1")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env("RUSTFLAGS", rustflags(&sysroot_dir))
        .status()
        .context("Failed to create sysroot cargo project")?
        .success();

    ensure!(success, "Failed to build sysroot");

    // Use cargo check to get the list of artifacts
    let metadata = cargo("check")
        .arg("--message-format=json-render-diagnostics")
        .arg("--quiet")
        .target(&args.target)
        .manifest_path(&Some(crate_dir.join("Cargo.toml")))
        .target_dir(&target_dir)
        .arg("-Zbuild-std=core,alloc")
        .arg("-Zbuild-std-features=compiler_builtins/mem")
        .arg("--release")
        .env("RUSTC_BOOTSTRAP", "1")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env("RUSTFLAGS", rustflags(&sysroot_dir))
        .output()
        .context("Failed to create sysroot cargo project")?;

    ensure!(metadata.status.success(), "Failed to build sysroot");

    let metadata = String::from_utf8_lossy(&metadata.stdout);
    let mut artifacts = vec![];
    for line in metadata.lines() {
        let Ok(metadata) = serde_json::from_str::<CargoCheck>(line) else {
            continue;
        };
        artifacts.extend(metadata.filenames.into_iter().filter_map(|f| {
            let filename = f.file_name()?.to_str()?;
            let (stem, ext) = filename.rsplit_once('.')?;
            let (stem, _) = stem.split_once('-')?;
            // skip libsysroot as they are for our empty dummy crate
            if stem != "libsysroot" && (ext == "rlib" || ext == "rmeta") {
                Some(f)
            } else {
                None
            }
        }));
    }

    let lib_dir = triplet_dir.join("lib");
    std::fs::create_dir_all(&lib_dir).context("Failed to create sysroot lib directory")?;

    // Find any old artifacts in the sysroot lib directory
    let to_remove = lib_dir
        .read_dir()
        .context("Failed to read sysroot lib directory")?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let filename = path.file_name()?;
            artifacts
                .iter()
                .any(|file| file.file_name() == Some(filename))
                .not()
                .then_some(path)
        });

    // Remove old artifacts
    for artifact in to_remove {
        std::fs::remove_file(artifact).context("Failed to remove old sysroot artifact")?;
    }

    // Copy new artifacts
    for artifact in artifacts {
        let filename = artifact.file_name().unwrap();
        let dst = lib_dir.join(filename);
        if !dst.exists() {
            std::fs::copy(&artifact, dst).context("Failed to copy sysroot artifact")?;
        }
    }

    Ok(sysroot_dir)
}

pub fn rustflags(sysroot_dir: impl AsRef<Path>) -> OsString {
    let mut env = std::env::var_os("RUSTFLAGS").unwrap_or_default();
    env.push(" --sysroot=");
    env.push(sysroot_dir.as_ref());
    env
}

fn get_spec(args: &Args, triplet: impl AsRef<str>) -> Result<TargetSpec> {
    let output = cargo("rustc")
        .target(triplet)
        .manifest_path(&args.manifest_path)
        .arg("-Zunstable-options")
        .arg("--print=target-spec-json")
        .arg("--")
        .arg("-Zunstable-options")
        .env("RUSTC_BOOTSTRAP", "1")
        .output()
        .context("Failed to get base target spec")?;
    ensure!(
        output.status.success(),
        "Failed to get base target spec: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).context("Failed to parse target spec JSON")
}
