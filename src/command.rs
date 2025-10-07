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

/// A process builder for cargo commands, providing a similar API to `std::process::Command`.
///
/// `CargoCommand` is a wrapper around `std::process::Command` specifically designed for
/// executing cargo commands targeting [hyperlight](https://github.com/hyperlight-dev/hyperlight)
/// guest code.
/// Before executing the desired command, `CargoCommand` takes care of setting up the
/// appropriate environment. It:
/// * creates a custom rust target for hyperlight guest code
/// * creates a sysroot with Rust's libs core and alloc
/// * finds the appropriate compiler and archiver for any C dependencies
/// * sets up necessary environment variables for `cc-rs` and `bindgen` to work correctly.
///
/// # Examples
///
/// Basic usage:
///
/// ```rust
/// use cargo_hyperlight::CargoCommand;
///
/// let mut command = CargoCommand::new();
/// command.arg("build").arg("--release");
/// command.exec(); // This will replace the current process
/// ```
///
/// Setting environment variables and working directory:
///
/// ```rust
/// use cargo_hyperlight::CargoCommand;
///
/// let mut command = CargoCommand::new();
/// command
///     .current_dir("/path/to/project")
///     .env("CARGO_TARGET_DIR", "/custom/target")
///     .args(["build", "--release"]);
/// ```
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
    /// Constructs a new `CargoCommand` for launching the cargo program.
    ///
    /// The value of the `CARGO` environment variable is used if it is set; otherwise, the
    /// default `cargo` from binary `PATH` is used.
    /// If `RUSTUP_TOOLCHAIN` is set in the environment, it is also propagated to the
    /// child process to ensure correct functioning of the rustup wrappers.
    ///
    /// The default configuration is:
    /// - No arguments to the program
    /// - Inherits the current process's environment
    /// - Inherits the current process's working directory
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```rust
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// let command = CargoCommand::new();
    /// ```
    pub fn new() -> Self {
        CargoCommand {
            command: cargo(),
            clear_env: false,
        }
    }

    /// Adds an argument to pass to the cargo program.
    ///
    /// Only one argument can be passed per use. So instead of:
    ///
    /// ```no_run
    /// command.arg("-C /path/to/repo");
    /// ```
    ///
    /// usage would be:
    ///
    /// ```no_run
    /// command.arg("-C").arg("/path/to/repo");
    /// ```
    ///
    /// To pass multiple arguments see [`args`].
    ///
    /// [`args`]: CargoCommand::args
    ///
    /// Note that the argument is not shell-escaped, so if you pass an argument like
    /// `"hello world"`, it will be passed as a single argument with the literal
    /// `hello world`, not as two arguments `hello` and `world`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// CargoCommand::new()
    ///         .arg("build")
    ///         .arg("--release")
    ///         .exec();
    /// ```
    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.command.arg(arg.as_ref());
        self
    }

    /// Adds multiple arguments to pass to the cargo program.
    ///
    /// To pass a single argument see [`arg`].
    ///
    /// [`arg`]: CargoCommand::arg
    ///
    /// Note that the arguments are not shell-escaped, so if you pass an argument
    /// like `"hello world"`, it will be passed as a single argument with the
    /// literal `hello world`, not as two arguments `hello` and `world`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// CargoCommand::new()
    ///         .args(["build", "--release"])
    ///         .exec();
    /// ```
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// CargoCommand::new()
    ///         .args(&["build", "--release"])
    ///         .exec();
    /// ```
    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> &mut Self {
        self.command.args(args);
        self
    }

    /// Sets the working directory for the child process.
    ///
    /// # Platform-specific behavior
    ///
    /// If the program path is relative (e.g., `"./script.sh"`), it's ambiguous
    /// whether it should be interpreted relative to the parent's working
    /// directory or relative to `current_dir`. The behavior in this case is
    /// platform specific and unstable, and it's recommended to use
    /// [`canonicalize`] to get an absolute program path instead.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// CargoCommand::new()
    ///         .current_dir("/bin")
    ///         .arg("build")
    ///         .exec();
    /// ```
    ///
    /// [`canonicalize`]: std::fs::canonicalize
    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
        self.command.current_dir(dir);
        self
    }

    /// Inserts or updates an explicit environment variable mapping.
    ///
    /// This method allows you to add an environment variable mapping to the spawned process
    /// or overwrite a variable if it already exists.
    ///
    /// Child processes will inherit environment variables from their parent process by
    /// default (unless [`env_clear`] is used), including variables set with this method.
    /// Environment variable mappings added or updated with this method will take
    /// precedence over inherited variables.
    ///
    /// Note that environment variable names are case-insensitive (but
    /// case-preserving) on Windows and case-sensitive on all other platforms.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// CargoCommand::new()
    ///         .env("PATH", "/bin")
    ///         .arg("build")
    ///         .exec();
    /// ```
    ///
    /// [`env_clear`]: CargoCommand::env_clear
    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.command.env(key, value);
        self
    }

    /// Clears all environment variables that will be set for the child process.
    ///
    /// This method will remove all environment variables from the child process,
    /// including those that would normally be inherited from the parent process.
    /// Environment variables can be added back individually using [`env`].
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// CargoCommand::new()
    ///         .env_clear()
    ///         .env("PATH", "/bin")
    ///         .arg("build")
    ///         .exec();
    /// ```
    ///
    /// [`env`]: CargoCommand::env
    pub fn env_clear(&mut self) -> &mut Self {
        let rust_toolchain = self
            .get_envs()
            .find_map(|(k, v)| (k == "RUSTUP_TOOLCHAIN").then_some(v))
            .flatten()
            .map(|v| v.to_os_string());
        self.clear_env = true;
        self.command.env_clear();
        if let Some(rust_toolchain) = rust_toolchain {
            self.command.env("RUSTUP_TOOLCHAIN", rust_toolchain);
        }
        self
    }

    /// Removes an explicitly set environment variable and prevents inheriting
    /// it from a parent process.
    ///
    /// This method will ensure that the specified environment variable is not
    /// present in the spawned process's environment, even if it was present
    /// in the parent process. This serves to "unset" environment variables.
    ///
    /// Note that environment variable names are case-insensitive (but
    /// case-preserving) on Windows and case-sensitive on all other platforms.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// CargoCommand::new()
    ///         .env_remove("PATH")
    ///         .arg("build")
    ///         .exec();
    /// ```
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.command.env_remove(key);
        self
    }

    /// Inserts or updates multiple explicit environment variable mappings.
    ///
    /// This method allows you to add multiple environment variable mappings
    /// to the spawned process or overwrite variables if they already exist.
    /// Environment variables can be passed as a `HashMap` or any other type
    /// implementing `IntoIterator` with the appropriate item type.
    ///
    /// Child processes will inherit environment variables from their parent process by
    /// default (unless [`env_clear`] is used), including variables set with this method.
    /// Environment variable mappings added or updated with this method will take
    /// precedence over inherited variables.
    ///
    /// Note that environment variable names are case-insensitive (but
    /// case-preserving) on Windows and case-sensitive on all other platforms.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use std::collections::HashMap;
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// let mut envs = HashMap::new();
    /// envs.insert("PATH", "/bin");
    /// envs.insert("CARGO_HOME", "/tmp/cargo");
    ///
    /// CargoCommand::new()
    ///         .envs(&envs)
    ///         .arg("build")
    ///         .exec();
    /// ```
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// CargoCommand::new()
    ///         .envs([("PATH", "/bin"), ("CARGO_HOME", "/tmp/cargo")])
    ///         .arg("build")
    ///         .exec();
    /// ```
    ///
    /// [`env_clear`]: CargoCommand::env_clear
    pub fn envs(
        &mut self,
        envs: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
    ) -> &mut Self {
        self.command.envs(envs);
        self
    }

    /// Returns an iterator over the arguments that will be passed to the cargo program.
    ///
    /// This does not include the program name itself (which can be retrieved with
    /// [`get_program`]).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// let mut command = CargoCommand::new();
    /// command.arg("build").arg("--release");
    ///
    /// let args: Vec<&std::ffi::OsStr> = command.get_args().collect();
    /// assert_eq!(args, &["build", "--release"]);
    /// ```
    ///
    /// [`get_program`]: CargoCommand::get_program
    pub fn get_args(&'_ self) -> CommandArgs<'_> {
        self.command.get_args()
    }

    /// Returns the working directory for the child process.
    ///
    /// This returns `None` if the working directory will not be changed from
    /// the current directory of the parent process.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::Path;
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// let mut command = CargoCommand::new();
    /// assert_eq!(command.get_current_dir(), None);
    ///
    /// command.current_dir("/bin");
    /// assert_eq!(command.get_current_dir(), Some(Path::new("/bin")));
    /// ```
    pub fn get_current_dir(&self) -> Option<&Path> {
        self.command.get_current_dir()
    }

    /// Returns an iterator over the environment mappings that will be set for the child process.
    ///
    /// Environment variables explicitly set or unset via [`env`], [`envs`], and
    /// [`env_remove`] can be retrieved with this method.
    ///
    /// Note that this output does not include environment variables inherited from the
    /// parent process.
    ///
    /// Each element is a tuple key/value where `None` means the variable is explicitly
    /// unset in the child process environment.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::ffi::OsStr;
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// let mut command = CargoCommand::new();
    /// command.env("CARGO_HOME", "/tmp/cargo");
    /// command.env_remove("PATH");
    ///
    /// for (key, value) in command.get_envs() {
    ///     println!("{key:?} => {value:?}");
    /// }
    /// ```
    ///
    /// [`env`]: CargoCommand::env
    /// [`envs`]: CargoCommand::envs
    /// [`env_remove`]: CargoCommand::env_remove
    pub fn get_envs(&'_ self) -> CommandEnvs<'_> {
        self.command.get_envs()
    }

    /// Returns the base environment variables for the command.
    ///
    /// This method returns the environment variables that will be inherited
    /// from the current process, taking into account whether [`env_clear`] has been called.
    ///
    /// [`env_clear`]: CargoCommand::env_clear
    fn base_env(&self) -> VarsOs {
        let mut env = env::vars_os();
        if self.clear_env {
            env.find(|_| false);
        }
        env
    }

    /// Resolves the final environment variables that will be passed to the cargo process.
    ///
    /// This combines the base environment with any explicitly set environment variables.
    fn resolve_envs(&self) -> HashMap<OsString, OsString> {
        self.command.resolve_env(self.base_env())
    }

    /// Returns the path to the cargo program that will be executed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// let command = CargoCommand::new();
    /// println!("Program: {}", command.get_program().to_string_lossy());
    /// ```
    pub fn get_program(&self) -> &OsStr {
        self.command.get_program()
    }

    /// Prepares the sysroot for the cargo command.
    ///
    /// This is an internal method that sets up any necessary sysroot configuration
    /// before executing the cargo command.
    fn prepare_sysroot(&mut self) -> anyhow::Result<()> {
        self.command.prepare_sysroot(self.base_env())?;
        Ok(())
    }

    /// Executes a cargo command as a child process, waiting for it to finish and
    /// collecting its exit status.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// let result = CargoCommand::new()
    ///     .arg("build")
    ///     .status();
    ///
    /// match result {
    ///     Ok(()) => println!("Cargo command succeeded"),
    ///     Err(e) => println!("Cargo command failed: {}", e),
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// This method will return an error if:
    /// - The sysroot preparation fails
    /// - The cargo process could not be spawned
    /// - The cargo process returned a non-zero exit status
    pub fn status(&mut self) -> anyhow::Result<()> {
        self.prepare_sysroot()
            .context("Failed to prepare sysroot")?;

        self.command
            .checked_status()
            .context("Failed to execute cargo")
    }

    /// Executes the cargo command, replacing the current process.
    ///
    /// This function will never return on success, as it replaces the current process
    /// with the cargo process. On error, it will print the error and exit with code 101.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use cargo_hyperlight::CargoCommand;
    ///
    /// CargoCommand::new()
    ///     .arg("build")
    ///     .exec(); // This will never return
    /// ```
    ///
    /// # Panics
    ///
    /// This function will exit the process with code 101 if:
    /// - The sysroot preparation fails
    /// - The process replacement fails
    pub fn exec(&mut self) -> ! {
        match self.exec_impl() {
            Err(e) => {
                eprintln!("{e:?}");
                std::process::exit(101);
            }
        }
    }

    /// Internal implementation of process replacement.
    ///
    /// This method prepares the sysroot and then calls the low-level `exec` function
    /// to replace the current process.
    fn exec_impl(&mut self) -> anyhow::Result<Infallible> {
        self.prepare_sysroot()
            .context("Failed to prepare sysroot")?;

        if let Some(cwd) = self.get_current_dir() {
            env::set_current_dir(cwd).context("Failed to change current directory")?;
        }

        Ok(exec(
            self.get_program(),
            self.get_args(),
            self.resolve_envs(),
        )?)
    }
}

/// Replaces the current process with the specified program using `execvpe`.
///
/// This function converts the provided arguments and environment variables into
/// the format expected by the `execvpe` system call and then replaces the current
/// process with the new program.
///
/// # Arguments
///
/// * `program` - The path to the program to execute
/// * `args` - The command-line arguments to pass to the program
/// * `envs` - The environment variables to set for the new process
///
/// # Returns
///
/// This function should never return on success. On failure, it returns an
/// `std::io::Error` describing what went wrong.
///
/// # Safety
///
/// This function uses unsafe code to call `libc::execvpe`. The implementation
/// carefully manages memory to ensure null-terminated strings are properly
/// constructed for the system call.
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
