use std::ops::Not as _;
use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use target_spec_json::TargetSpec;

use crate::cargo::{CargoCmd, cargo};
use crate::cli::Args;

const CARGO_TOML: &str = include_str!("dummy/_Cargo.toml");
const LIB_RS: &str = include_str!("dummy/_lib.rs");

#[derive(serde::Deserialize)]
struct Invocation {
    outputs: Vec<PathBuf>,
}

#[derive(serde::Deserialize)]
struct BuildPlan {
    invocations: Vec<Invocation>,
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

    let sysroot_dir = args.sysroot_dir();
    let target_dir = args.build_dir();
    let triplet_dir = args.triplet_dir();
    let crate_dir = args.crate_dir();
    let lib_dir = args.libs_dir();
    let build_plan_dir = args.build_plan_dir();

    std::fs::create_dir_all(&triplet_dir).context("Failed to create sysroot directories")?;
    std::fs::write(
        triplet_dir.join("target.json"),
        serde_json::to_string_pretty(&target_spec).unwrap(),
    )
    .context("Failed to write target spec file")?;

    let version = cargo()?
        .env_clear()
        .envs(args.env.iter())
        .current_dir(&args.current_dir)
        .arg("version")
        .arg("--verbose")
        .checked_output()
        .context("Failed to get cargo version")?;

    let version = String::from_utf8_lossy(&version.stdout);
    let version = version
        .lines()
        .find_map(|l| l.trim().strip_prefix("release: "))
        .context("Failed to parse cargo version")?;

    let cargo_toml = CARGO_TOML.replace("0.0.0", version);

    std::fs::create_dir_all(&crate_dir).context("Failed to create target directory")?;
    std::fs::write(crate_dir.join("Cargo.toml"), cargo_toml)
        .context("Failed to write Cargo.toml")?;
    std::fs::write(crate_dir.join("lib.rs"), LIB_RS).context("Failed to write lib.rs")?;

    // if we are using rustup, ensure that the rust-src component is installed
    if let Some(rustup_toolchain) = std::env::var_os("RUSTUP_TOOLCHAIN") {
        std::process::Command::new("rustup")
            .arg("--quiet")
            .arg("component")
            .arg("add")
            .arg("rust-src")
            .arg("--toolchain")
            .arg(rustup_toolchain)
            .checked_output()
            .context("Failed to get Rust's std lib sources")?;
    }

    // Use cargo build's build plan to get the list of artifacts
    let build_plan = cargo()?
        .env_clear()
        .envs(args.env.iter())
        .current_dir(&args.current_dir)
        .arg("build")
        .arg("--quiet")
        .target(&args.target)
        .manifest_path(&Some(crate_dir.join("Cargo.toml")))
        .target_dir(&build_plan_dir)
        .arg("-Zbuild-std=core,alloc")
        .arg("-Zbuild-std-features=compiler_builtins/mem")
        .arg("--release")
        .arg("-Zunstable-options")
        .arg("--build-plan")
        // build-plan is an unstable feature
        .allow_unstable()
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .sysroot(&sysroot_dir)
        .checked_output()
        .context("Failed to build sysroot")?;

    let build_plan = String::from_utf8_lossy(&build_plan.stdout);
    let mut artifacts = vec![];
    for line in build_plan.lines() {
        let Ok(step) = serde_json::from_str::<BuildPlan>(line) else {
            continue;
        };
        artifacts.extend(
            step.invocations
                .into_iter()
                .flat_map(|i| i.outputs)
                .filter_map(|f| {
                    let Ok(f) = f.strip_prefix(&build_plan_dir) else {
                        return None;
                    };
                    let filename = f.file_name()?.to_str()?;
                    let (stem, ext) = filename.rsplit_once('.')?;
                    let (stem, _) = stem.split_once('-')?;
                    // skip libsysroot as they are for our empty dummy crate
                    if stem != "libsysroot" && (ext == "rlib" || ext == "rmeta") {
                        Some(target_dir.join(f))
                    } else {
                        None
                    }
                }),
        );
    }

    // check if any artifacts is missing
    let should_build = artifacts.iter().any(|f| !f.exists());

    if should_build {
        // Build the sysroot
        let success = cargo()?
            .env_clear()
            .envs(args.env.iter())
            .current_dir(&args.current_dir)
            .arg("build")
            .target(&args.target)
            .manifest_path(&Some(crate_dir.join("Cargo.toml")))
            .target_dir(&target_dir)
            .arg("-Zbuild-std=core,alloc")
            .arg("-Zbuild-std-features=compiler_builtins/mem")
            .arg("--release")
            // The core, alloc and compiler_builtins crates use unstable features
            .allow_unstable()
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .sysroot(&sysroot_dir)
            .status()
            .context("Failed to create sysroot cargo project")?
            .success();

        ensure!(success, "Failed to build sysroot");
    }

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

fn get_spec(args: &Args, triplet: impl AsRef<str>) -> Result<TargetSpec> {
    let output = cargo()?
        .env_clear()
        .envs(args.env.iter())
        .current_dir(&args.current_dir)
        .arg("rustc")
        .target(triplet)
        .manifest_path(&args.manifest_path)
        .arg("-Zunstable-options")
        .arg("--print=target-spec-json")
        .arg("--")
        .arg("-Zunstable-options")
        // printing target-spec-json is an unstable feature
        .allow_unstable()
        .checked_output()
        .context("Failed to get base target spec")?;

    serde_json::from_slice(&output.stdout).context("Failed to parse target spec JSON")
}
