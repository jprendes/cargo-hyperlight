use std::ffi::{OsStr, OsString};
use std::path::Path;

pub trait CargoCmd {
    fn manifest_path(&mut self, path: &Option<impl AsRef<Path>>) -> &mut Self;
    fn target_dir(&mut self, path: impl AsRef<Path>) -> &mut Self;
    fn target(&mut self, triplet: impl AsRef<str>) -> &mut Self;
    fn cc_env(&mut self, triplet: impl AsRef<str>, cc: impl AsRef<Path>) -> &mut Self;
    fn ar_env(&mut self, triplet: impl AsRef<str>, ar: impl AsRef<Path>) -> &mut Self;
    fn sysroot(&mut self, path: impl AsRef<Path>) -> &mut Self;
    fn append_rustflags(&mut self, flags: impl AsRef<OsStr>) -> &mut Self;
    fn append_cflags(&mut self, triplet: impl AsRef<str>, flags: impl AsRef<OsStr>) -> &mut Self;
    fn append_bindgen_cflags(&mut self, flags: impl AsRef<OsStr>) -> &mut Self;
}

pub trait CargoCmdExt {
    fn allow_unstable(&mut self) -> &mut Self;
}

pub fn cargo() -> std::process::Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    std::process::Command::new(cargo)
}

impl CargoCmd for std::process::Command {
    fn manifest_path(&mut self, path: &Option<impl AsRef<Path>>) -> &mut Self {
        if let Some(path) = path {
            self.arg("--manifest-path").arg(path.as_ref());
        }
        self
    }

    fn target_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.env("CARGO_BUILD_TARGET_DIR", path.as_ref());
        self.env("CARGO_TARGET_DIR", path.as_ref());
        self
    }

    fn target(&mut self, triplet: impl AsRef<str>) -> &mut Self {
        self.env("CARGO_BUILD_TARGET", triplet.as_ref());
        self
    }

    fn cc_env(&mut self, triplet: impl AsRef<str>, cc: impl AsRef<Path>) -> &mut Self {
        // set both CC_<triplet> and CLANG_PATH so that cc-rs and bindgen can pick it up
        // use CC_<triplet> as this is the highest priority for cc-rs
        // see https://docs.rs/cc/latest/cc/#external-configuration-via-environment-variables
        self.env(format!("CC_{}", triplet.as_ref()), cc.as_ref());
        self.env("CLANG_PATH", cc.as_ref());
        self
    }

    fn ar_env(&mut self, triplet: impl AsRef<str>, ar: impl AsRef<Path>) -> &mut Self {
        // set AR_<triplet> so that cc-rs can pick it up
        self.env(format!("AR_{}", triplet.as_ref()), ar.as_ref());
        self
    }

    fn sysroot(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.append_rustflags("--sysroot")
            .append_rustflags(path.as_ref())
    }

    fn append_rustflags(&mut self, flags: impl AsRef<OsStr>) -> &mut Self {
        if flags.as_ref().is_empty() {
            return self;
        }

        let mut new_flags = get_env(self, "RUSTFLAGS").unwrap_or_default();
        if !new_flags.is_empty() {
            new_flags.push(" ");
        }
        new_flags.push(flags.as_ref());
        self.env("RUSTFLAGS", new_flags);
        self
    }

    fn append_cflags(&mut self, triplet: impl AsRef<str>, flags: impl AsRef<OsStr>) -> &mut Self {
        if flags.as_ref().is_empty() {
            return self;
        }

        let triplet = triplet.as_ref();
        let triplet_snake_case = triplet.replace('-', "_");
        let triplet_snake_case_upper = triplet_snake_case.to_uppercase();

        let search_keys = [
            format!("CFLAGS_{triplet}"),
            format!("CFLAGS_{triplet_snake_case}"),
            format!("CFLAGS_{triplet_snake_case_upper}"),
            "HYPERLIGHT_CFLAGS".to_string(),
            "TARGET_CFLAGS".to_string(),
            "CFLAGS".to_string(),
        ];

        let mut new_flags = search_keys
            .iter()
            .find_map(|key| get_env(self, key))
            .unwrap_or_default();

        if !new_flags.is_empty() {
            new_flags.push(" ");
        }
        new_flags.push(flags.as_ref());
        self.env(&search_keys[0], new_flags);

        self.append_bindgen_cflags(flags);

        self
    }

    fn append_bindgen_cflags(&mut self, flags: impl AsRef<OsStr>) -> &mut Self {
        if flags.as_ref().is_empty() {
            return self;
        }

        let mut new_flags = get_env(self, "BINDGEN_EXTRA_CLANG_ARGS").unwrap_or_default();
        if !new_flags.is_empty() {
            new_flags.push(" ");
        }
        new_flags.push(flags.as_ref());
        self.env("BINDGEN_EXTRA_CLANG_ARGS", new_flags);
        self
    }
}

fn get_env(cmd: &std::process::Command, key: &str) -> Option<OsString> {
    let mut envs = cmd.get_envs();
    match envs.find(|(k, _)| *k == key) {
        Some((_, v)) => v.map(ToOwned::to_owned),
        None => std::env::var_os(key),
    }
}

impl CargoCmdExt for std::process::Command {
    fn allow_unstable(&mut self) -> &mut Self {
        self.env("RUSTC_BOOTSTRAP", "1")
    }
}
