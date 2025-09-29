use std::{
    env,
    ffi::OsString,
    iter::once,
    path::{Path, PathBuf},
};

use regex::Regex;

use crate::sysroot;

pub fn prepare(
    target_dir: impl AsRef<Path>,
    manifest_path: impl AsRef<Path>,
    target: impl AsRef<str>,
) -> PathBuf {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target_dir = target_dir.as_ref();
    let toolchain_dir = target_dir.join("toolchain");
    let sysroot_dir = target_dir.join("sysroot");

    if toolchain_dir.join("clang").exists() {
        return toolchain_dir;
    }

    let success = std::process::Command::new(cargo)
        .arg("build")
        .arg("--release")
        // Add target triplet
        .arg("--target")
        .arg(&target.as_ref())
        // Add manifest-path
        .arg("--manifest-path")
        .arg(&manifest_path.as_ref())
        // Add target-dir
        .arg("--target-dir")
        .arg(&target_dir)
        // Build hyperlight-guest-bin
        .arg("--package")
        .arg("hyperlight-guest-bin")
        .env("HYPERLIGHT_GUEST_TOOLCHAIN_ROOT", &toolchain_dir)
        .env("RUSTFLAGS", sysroot::rustflags(&sysroot_dir))
        .status()
        .expect("Failed to execute cargo")
        .success();

    assert!(success, "Failed to prepare toolchain");

    toolchain_dir
}

pub fn path_with(toolchain: impl Into<PathBuf>) -> OsString {
    let path = toolchain.into();
    let paths = env::var_os("PATH").unwrap_or_default();
    let paths = env::split_paths(&paths);
    let paths = once(path).chain(paths);
    env::join_paths(paths).unwrap()
}

pub fn cflags(triplet: impl AsRef<str>) -> OsString {
    let mut env = get_cflags(triplet);
    env.push(" -fPIC");
    env
}

fn get_cflags(triplet: impl AsRef<str>) -> OsString {
    let triplet = triplet.as_ref();
    if let Some(cflags) = std::env::var_os(format!("CFLAGS_{triplet}")) {
        return cflags;
    }
    let triplet_snake_case = triplet.replace('-', "_");
    if let Some(cflags) = std::env::var_os(format!("CFLAGS_{triplet_snake_case}")) {
        return cflags;
    }
    if let Some(cflags) = std::env::var_os("HYPERLIGHT_CFLAGS") {
        return cflags;
    }
    if let Some(cflags) = std::env::var_os("TARGET_CFLAGS") {
        return cflags;
    }
    if let Some(cflags) = std::env::var_os("CFLAGS") {
        return cflags;
    }

    OsString::new()
}

pub fn find_cc(toolchain: impl AsRef<Path>) -> PathBuf {
    // check for clang in the toolchain directory
    let toolchain = toolchain.as_ref();
    let extension = if cfg!(windows) { "exe" } else { "" };
    let clang = toolchain.join("clang").with_extension(extension);
    assert!(clang.exists(), "Could not find clang in {clang:?}");
    clang
}

pub fn find_ar() -> PathBuf {
    if let Ok(path) = which::which("ar") {
        return path;
    }
    if let Ok(path) = which::which("llvm-ar") {
        return path;
    }
    // try with postfixed version llvm-ar, e.g., llvm-ar-20
    let re = Regex::new(r"llvm-ar-\d+").unwrap();
    which::which_re(&re)
        .map(|mut it| it.next())
        .ok()
        .flatten()
        .expect("Could not find 'ar' or 'llvm-ar' in PATH")
}
