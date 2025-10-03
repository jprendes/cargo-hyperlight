use std::collections::HashMap;
use std::convert::Infallible;
use std::env::VarsOs;
use std::ffi::{OsStr, OsString, c_char};
use std::fmt::Debug;
use std::path::Path;
use std::process::{Command, CommandArgs, CommandEnvs};
use std::{env, iter};

use anyhow::Context;

use crate::CargoCommandExt;
use crate::cargo::{CargoCmd as _, cargo};

pub struct CargoCommand {
    command: Command,
    clear_env: bool,
}

impl Debug for CargoCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.command, f)
    }
}

impl Default for CargoCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CargoCommand {
    pub fn new() -> Self {
        CargoCommand {
            command: cargo(),
            clear_env: false,
        }
    }

    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.command.arg(arg.as_ref());
        self
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> &mut Self {
        self.command.args(args);
        self
    }

    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
        self.command.current_dir(dir);
        self
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.command.env(key, value);
        self
    }

    pub fn env_clear(&mut self) -> &mut Self {
        self.clear_env = true;
        self.command.env_clear();
        self
    }

    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.command.env_remove(key);
        self
    }

    pub fn envs(
        &mut self,
        envs: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
    ) -> &mut Self {
        self.command.envs(envs);
        self
    }

    pub fn get_args(&'_ self) -> CommandArgs<'_> {
        self.command.get_args()
    }

    pub fn get_current_dir(&self) -> Option<&Path> {
        self.command.get_current_dir()
    }

    pub fn get_envs(&'_ self) -> CommandEnvs<'_> {
        self.command.get_envs()
    }

    fn base_env(&self) -> VarsOs {
        let mut env = env::vars_os();
        if self.clear_env {
            env.find(|_| false);
        }
        env
    }

    fn resolve_envs(&self) -> HashMap<OsString, OsString> {
        self.command.resolve_env(self.base_env())
    }

    pub fn get_program(&self) -> &OsStr {
        self.command.get_program()
    }

    fn prepare_sysroot(&mut self) -> anyhow::Result<()> {
        self.command.prepare_sysroot(self.base_env())?;
        Ok(())
    }

    pub fn status(&mut self) -> anyhow::Result<()> {
        self.prepare_sysroot()
            .context("Failed to prepare sysroot")?;

        self.command
            .checked_status()
            .context("Failed to execute cargo")
    }

    pub fn exec(&mut self) -> ! {
        match self.exec_impl() {
            Err(e) => {
                eprintln!("{e:?}");
                std::process::exit(101);
            }
        }
    }

    fn exec_impl(&mut self) -> anyhow::Result<Infallible> {
        self.prepare_sysroot()
            .context("Failed to prepare sysroot")?;

        Ok(exec(
            self.get_program(),
            self.get_args(),
            self.resolve_envs(),
        )?)
    }
}

fn exec(
    program: impl AsRef<OsStr>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    envs: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
) -> std::io::Result<Infallible> {
    let mut env_bytes = vec![];
    let mut env_offsets = vec![];
    for (k, v) in envs.into_iter() {
        env_offsets.push(env_bytes.len());
        env_bytes.extend_from_slice(k.as_ref().as_encoded_bytes());
        env_bytes.push(b'=');
        env_bytes.extend_from_slice(v.as_ref().as_encoded_bytes());
        env_bytes.push(0);
    }
    let env_ptrs = env_offsets
        .into_iter()
        .map(|offset| env_bytes[offset..].as_ptr() as *const c_char)
        .chain(iter::once(std::ptr::null()))
        .collect::<Vec<_>>();

    let mut arg_bytes = vec![];
    let mut arg_offsets = vec![];

    arg_offsets.push(arg_bytes.len());
    arg_bytes.extend_from_slice(program.as_ref().as_encoded_bytes());
    arg_bytes.push(0);

    for arg in args {
        arg_offsets.push(arg_bytes.len());
        arg_bytes.extend_from_slice(arg.as_ref().as_encoded_bytes());
        arg_bytes.push(0);
    }
    let arg_ptrs = arg_offsets
        .into_iter()
        .map(|offset| arg_bytes[offset..].as_ptr() as *const c_char)
        .chain(iter::once(std::ptr::null()))
        .collect::<Vec<_>>();

    unsafe { libc::execvpe(arg_ptrs[0], arg_ptrs.as_ptr(), env_ptrs.as_ptr()) };

    Err(std::io::Error::last_os_error())
}
