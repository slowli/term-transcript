//! Standard shell support.

use std::{
    ffi::OsStr,
    io,
    path::Path,
    process::{Child, ChildStdin, Command},
};

use term_style::StyledStr;

use super::ShellOptions;
use crate::{
    ExitStatus,
    traits::{ConfigureCommand, Echoing, SpawnShell, SpawnedShell},
};

#[derive(Debug, Clone, Copy)]
enum StdShellType {
    /// `sh` shell.
    Sh,
    /// `bash` shell.
    Bash,
    /// PowerShell.
    PowerShell,
}

/// Shell interpreter that brings additional functionality for [`ShellOptions`].
#[derive(Debug)]
pub struct StdShell {
    shell_type: StdShellType,
    command: Command,
}

impl ConfigureCommand for StdShell {
    fn current_dir(&mut self, dir: &Path) {
        self.command.current_dir(dir);
    }

    fn env(&mut self, name: &str, value: &OsStr) {
        self.command.env(name, value);
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", ret))]
fn check_sh_exit_code(response: StyledStr<'_>) -> Option<ExitStatus> {
    let response = response.text();
    response.trim().parse().ok().map(ExitStatus)
}

#[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", ret))]
fn check_ps_exit_code(response: StyledStr<'_>) -> Option<ExitStatus> {
    let response = response.text();
    match response.trim() {
        "True" => Some(ExitStatus(0)),
        "False" => Some(ExitStatus(1)),
        _ => None,
    }
}

impl ShellOptions<StdShell> {
    /// Creates options for an `sh` shell.
    pub fn sh() -> Self {
        let this = Self::new(StdShell {
            shell_type: StdShellType::Sh,
            command: Command::new("sh"),
        });
        this.with_status_check("echo $?", check_sh_exit_code)
    }

    /// Creates options for a Bash shell.
    pub fn bash() -> Self {
        let this = Self::new(StdShell {
            shell_type: StdShellType::Bash,
            command: Command::new("bash"),
        });
        this.with_status_check("echo $?", check_sh_exit_code)
    }

    /// Creates options for PowerShell 6+ (the one with the `pwsh` executable).
    pub fn pwsh() -> Self {
        let mut command = Command::new("pwsh");
        command.arg("-NoLogo").arg("-NoExit");

        let command = StdShell {
            shell_type: StdShellType::PowerShell,
            command,
        };
        Self::new(command)
            .with_init_command("function prompt { }")
            .with_status_check("echo $?", check_ps_exit_code)
    }

    /// Creates an alias for the binary at `path_to_bin`, which should be an absolute path.
    /// This allows to call the binary using this alias without complex preparations (such as
    /// installing it globally via `cargo install`), and is more flexible than
    /// [`Self::with_cargo_path()`].
    ///
    /// In integration tests, you may use [`env!("CARGO_BIN_EXE_<name>")`] to get a path
    /// to binary targets.
    ///
    /// # Limitations
    ///
    /// - For Bash and PowerShell, `name` must be a valid name of a function. For `sh`,
    ///   `name` must be a valid name for the `alias` command. The `name` validity
    ///   is **not** checked.
    ///
    /// [`env!("CARGO_BIN_EXE_<name>")`]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
    #[must_use]
    pub fn with_alias(self, name: &str, path_to_bin: &str) -> Self {
        let alias_command = match self.command.shell_type {
            StdShellType::Sh => {
                format!("alias {name}=\"'{path_to_bin}'\"")
            }
            StdShellType::Bash => format!("{name}() {{ '{path_to_bin}' \"$@\"; }}"),
            StdShellType::PowerShell => format!("function {name} {{ & '{path_to_bin}' @Args }}"),
        };

        self.with_init_command(alias_command)
    }
}

impl SpawnShell for StdShell {
    type ShellProcess = Echoing<Child>;
    type Reader = os_pipe::PipeReader;
    type Writer = ChildStdin;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", err))]
    fn spawn_shell(&mut self) -> io::Result<SpawnedShell<Self>> {
        let SpawnedShell {
            shell,
            reader,
            writer,
        } = self.command.spawn_shell()?;

        let is_echoing = matches!(self.shell_type, StdShellType::PowerShell);
        Ok(SpawnedShell {
            shell: Echoing::new(shell, is_echoing),
            reader,
            writer,
        })
    }
}
