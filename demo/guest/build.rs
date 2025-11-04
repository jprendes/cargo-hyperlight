fn main() {
    println!("cargo:rerun-if-changed=src/host_print.c");
    println!("cargo:rerun-if-changed=src/host_print.h");

    cc::Build::new()
        .file("src/host_print.c")
        .compile("host_print");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_dir = std::path::PathBuf::from(out_dir);
    bindgen::Builder::default()
        .header("src/host_print.h")
        .use_core()
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings");
}