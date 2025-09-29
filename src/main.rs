use std::process::Command;

mod cli;
mod sysroot;
mod toolchain;

fn main() {
    let args = cli::Args::parse();

    // Build sysroot
    let sysroot = sysroot::build(&args.target_dir, &args.target);

    // Build toolchain
    let toolchain = toolchain::prepare(&args.target_dir, &args.manifest_path, &args.target);

    let triplet = args.target;

    // Execute cargo
    let status = Command::new("cargo")
        .arg(args.command)
        // Add target triplet
        .arg("--target")
        .arg(&triplet)
        // Add manifest-path
        .arg("--manifest-path")
        .arg(&args.manifest_path)
        // Add target-dir
        .arg("--target-dir")
        .arg(&args.target_dir)
        // Add remaining arguments
        .args(&args.cargo_args)
        // Populate rustflags with sysroot and codegen options
        .env("RUSTFLAGS", sysroot::rustflags(&sysroot))
        // Add the toolchain to PATH
        .env("PATH", toolchain::path_with(&toolchain))
        // Set the hyperlight toolchain environment variables
        //.env("HYPERLIGHT_GUEST_TOOLCHAIN_ROOT", &toolchain)
        // Set CC so that cc-rs can pick it up
        .env(format!("CC_{triplet}"), toolchain.join("clang"))
        .env(format!("CFLAGS_{triplet}"), toolchain::cflags(&triplet))
        // set CLANG_PATH so that bindgen can pick it up
        .env("CLANG_PATH", toolchain.join("clang"))
        .status()
        .expect("Failed to execute cargo");

    // Exit with cargo's exit code
    std::process::exit(status.code().unwrap_or(1));
}
