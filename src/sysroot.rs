use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use target_spec_json::TargetSpec;

const CARGO_TOML: &str = include_str!("dummy/_Cargo.toml");
const LIB_RS: &str = include_str!("dummy/_lib.rs");

pub fn build(target_dir: impl AsRef<Path>, triplet: impl AsRef<str>) -> PathBuf {
    let triplet = triplet.as_ref();

    let target_spec = match triplet.as_ref() {
        "x86_64-hyperlight-none" => {
            let mut spec = get_spec("x86_64-unknown-none");
            spec.entry_name = Some("entrypoint".into());
            spec.code_model = Some("small".into());
            spec.linker = Some("rust-lld".into());
            spec.linker_flavor = Some("gnu-lld".into());
            spec.pre_link_args =
                Some([("gnu-lld".to_string(), vec!["-znostart-stop-gc".to_string()])].into());
            spec
        }
        _ => panic!("Unsupported target triple: {triplet}"),
    };

    let sysroot_dir = target_dir.as_ref().join("sysroot");
    let target_dir = sysroot_dir.join("target");
    let triplet_dir = sysroot_dir.join("lib").join("rustlib").join(triplet);
    let crate_dir = sysroot_dir.join("crate");

    std::fs::create_dir_all(&triplet_dir).expect("Failed to create sysroot directories");
    std::fs::write(
        &triplet_dir.join("target.json"),
        serde_json::to_string_pretty(&target_spec).unwrap(),
    )
    .expect("Failed to write target spec file");

    std::fs::create_dir_all(&crate_dir).expect("Failed to create target directory");
    std::fs::write(&crate_dir.join("Cargo.toml"), CARGO_TOML).expect("Failed to write Cargo.toml");
    std::fs::write(&crate_dir.join("lib.rs"), LIB_RS).expect("Failed to write lib.rs");

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());

    if let Some(rustup_toolchain) = std::env::var_os("RUSTUP_TOOLCHAIN") {
        let _ = std::process::Command::new("rustup")
            .arg("component")
            .arg("add")
            .arg("rust-src")
            .arg("--toolchain")
            .arg(rustup_toolchain)
            .status();
    }

    let success = std::process::Command::new(cargo)
        .arg("rustc")
        .arg("-Zbuild-std=core,alloc")
        .arg("-Zbuild-std-features=compiler_builtins/mem")
        .arg("--target")
        .arg(&triplet)
        .arg("--release")
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--manifest-path")
        .arg(crate_dir.join("Cargo.toml"))
        .env("RUSTC_BOOTSTRAP", "1")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env("RUSTFLAGS", rustflags(&sysroot_dir))
        .status()
        .expect("Failed to create sysroot cargo project")
        .success();

    assert!(success, "Failed to build sysroot");

    let artifacts_dir = target_dir.join(triplet).join("release").join("deps");
    let lib_dir = triplet_dir.join("lib");
    std::fs::create_dir_all(&lib_dir).expect("Failed to create sysroot lib directory");
    for file in
        std::fs::read_dir(&artifacts_dir).expect("Failed to read sysroot artifacts directory")
    {
        let file = file.expect("Failed to read sysroot artifact").path();
        if !file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref()
            .starts_with("lib")
        {
            continue;
        }
        std::fs::copy(&file, &lib_dir.join(file.file_name().unwrap()))
            .expect("Failed to copy sysroot rlib");
    }

    sysroot_dir
}

pub fn rustflags(sysroot_dir: impl AsRef<Path>) -> OsString {
    let mut env = std::env::var_os("RUSTFLAGS").unwrap_or_default();
    env.push(" --sysroot=");
    env.push(sysroot_dir.as_ref());
    env
}

fn get_spec(triplet: impl AsRef<str>) -> TargetSpec {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(cargo)
        .arg("rustc")
        .arg("-Zunstable-options")
        .arg("--print=target-spec-json")
        .arg("--target")
        .arg(triplet.as_ref())
        .arg("--")
        .arg("-Zunstable-options")
        .env("RUSTC_BOOTSTRAP", "1")
        .output()
        .expect("Failed to get base target spec");
    assert!(
        output.status.success(),
        "Failed to get base target spec: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).expect("Failed to parse target spec output");
    let output = output.trim();
    serde_json::from_str(output).expect("Failed to parse target spec JSON")
}
