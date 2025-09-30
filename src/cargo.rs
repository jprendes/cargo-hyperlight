use std::ffi::OsStr;
use std::path::Path;

pub trait CargoCmd {
    fn manifest_path(&mut self, path: &Option<impl AsRef<Path>>) -> &mut Self;
    fn target_dir(&mut self, path: impl AsRef<Path>) -> &mut Self;
    fn target(&mut self, triplet: impl AsRef<str>) -> &mut Self;
    fn cc_env(&mut self, triplet: impl AsRef<str>, cc: impl AsRef<Path>) -> &mut Self;
    fn ar_env(&mut self, triplet: impl AsRef<str>, ar: impl AsRef<Path>) -> &mut Self;
    fn cflags_env(&mut self, triplet: impl AsRef<str>, cflags: impl AsRef<OsStr>) -> &mut Self;
}

pub fn cargo(command: impl AsRef<str>) -> std::process::Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = std::process::Command::new(cargo);
    cmd.arg(command.as_ref());
    cmd
}

impl CargoCmd for std::process::Command {
    fn manifest_path(&mut self, path: &Option<impl AsRef<Path>>) -> &mut Self {
        if let Some(path) = path {
            self.arg("--manifest-path").arg(path.as_ref());
        }
        self
    }

    fn target_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.arg("--target-dir").arg(path.as_ref());
        self
    }

    fn target(&mut self, triplet: impl AsRef<str>) -> &mut Self {
        self.arg("--target").arg(triplet.as_ref());
        self
    }

    fn cc_env(&mut self, triplet: impl AsRef<str>, cc: impl AsRef<Path>) -> &mut Self {
        // set both CC_<triplet> and CLANG_PATH so that cc-rs and bindgen can pick it up
        self.env(format!("CC_{}", triplet.as_ref()), cc.as_ref());
        self.env("CLANG_PATH", cc.as_ref());
        self
    }

    fn ar_env(&mut self, triplet: impl AsRef<str>, ar: impl AsRef<Path>) -> &mut Self {
        // set AR_<triplet> so that cc-rs can pick it up
        self.env(format!("AR_{}", triplet.as_ref()), ar.as_ref());
        self
    }

    fn cflags_env(&mut self, triplet: impl AsRef<str>, cflags: impl AsRef<OsStr>) -> &mut Self {
        // set CFLAGS_<triplet> so that cc-rs can pick it up
        self.env(format!("CFLAGS_{}", triplet.as_ref()), cflags.as_ref());
        self
    }
}
