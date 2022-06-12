//! Standard shell support.

use std::{
    ffi::OsStr,
    io,
    path::Path,
    process::{ChildStdin, Command},
};

use super::ShellOptions;
use crate::traits::{ChildShell, ConfigureCommand, SpawnShell, SpawnedShell};

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

impl ShellOptions<StdShell> {
    /// Creates options for an `sh` shell.
    pub fn sh() -> Self {
        Self::new(StdShell {
            shell_type: StdShellType::Sh,
            command: Command::new("sh"),
        })
    }

    /// Creates options for a Bash shell.
    pub fn bash() -> Self {
        Self::new(StdShell {
            shell_type: StdShellType::Bash,
            command: Command::new("bash"),
        })
    }

    /// Creates options for PowerShell.
    #[allow(clippy::doc_markdown)] // false positive
    pub fn powershell() -> Self {
        let mut command = Command::new("powershell");
        command.arg("-NoLogo").arg("-NoExit");

        let command = StdShell {
            shell_type: StdShellType::PowerShell,
            command,
        };
        Self::new(command).with_init_command("function prompt { }")
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
    #[allow(clippy::doc_markdown)] // false positive
    #[must_use]
    pub fn with_alias(self, name: &str, path_to_bin: &str) -> Self {
        let alias_command = match self.command.shell_type {
            StdShellType::Sh => {
                format!("alias {name}=\"'{path}'\"", name = name, path = path_to_bin)
            }
            StdShellType::Bash => format!(
                "{name}() {{ '{path}' \"$@\"; }}",
                name = name,
                path = path_to_bin
            ),
            StdShellType::PowerShell => format!(
                "function {name} {{ & '{path}' @Args }}",
                name = name,
                path = path_to_bin
            ),
        };

        self.with_init_command(alias_command)
    }
}

impl SpawnShell for StdShell {
    type ShellProcess = ChildShell;
    type Reader = os_pipe::PipeReader;
    type Writer = ChildStdin;

    fn spawn_shell(&mut self) -> io::Result<SpawnedShell<Self>> {
        let SpawnedShell {
            mut shell,
            reader,
            writer,
        } = self.command.spawn_shell()?;

        if matches!(self.shell_type, StdShellType::PowerShell) {
            shell.set_echoing();
        }

        Ok(SpawnedShell {
            shell,
            reader,
            writer,
        })
    }
}
