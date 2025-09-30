use std::env;
use std::ffi::OsString;
use std::iter::once;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use regex::Regex;

use crate::cargo::{CargoCmd, cargo};
use crate::cli::Args;
use crate::sysroot;

pub fn prepare(args: &Args) -> Result<PathBuf> {
    let target_dir = &args.target_dir;
    let toolchain_dir = target_dir.join("toolchain");
    let sysroot_dir = target_dir.join("sysroot");

    if toolchain_dir.join("clang").exists() {
        return Ok(toolchain_dir);
    }

    let success = cargo("build")
        .manifest_path(&args.manifest_path)
        .target(&args.target)
        .target_dir(&args.target_dir)
        .arg("--release")
        // Build hyperlight-guest-bin
        .arg("--package")
        .arg("hyperlight-guest-bin")
        .env("HYPERLIGHT_GUEST_TOOLCHAIN_ROOT", &toolchain_dir)
        .env("RUSTFLAGS", sysroot::rustflags(&sysroot_dir))
        .status()
        .context("Failed to execute cargo")?
        .success();

    ensure!(success, "Failed to prepare toolchain");

    Ok(toolchain_dir)
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

pub fn find_cc(toolchain: impl AsRef<Path>) -> Result<PathBuf> {
    // check for clang in the toolchain directory
    let toolchain = toolchain.as_ref();
    let extension = if cfg!(windows) { "exe" } else { "" };
    let clang = toolchain.join("clang").with_extension(extension);
    ensure!(clang.exists(), "Could not find clang in {clang:?}");
    Ok(clang)
}

pub fn find_ar() -> Result<PathBuf> {
    if let Ok(path) = which::which("ar") {
        return Ok(path);
    }
    if let Ok(path) = which::which("llvm-ar") {
        return Ok(path);
    }
    // try with postfixed version llvm-ar, e.g., llvm-ar-20
    let re = Regex::new(r"llvm-ar-\d+").unwrap();
    which::which_re(&re)
        .context("Could not find 'ar' or 'llvm-ar' in PATH")?
        .next()
        .context("Could not find 'ar' or 'llvm-ar' in PATH")
}
