//! Shell-related command-line args.

use structopt::StructOpt;

use std::{ffi::OsString, io, process::Command, time::Duration};

#[cfg(feature = "portable-pty")]
use term_transcript::PtyCommand;
use term_transcript::{traits::Echoing, ExitStatus, ShellOptions, Transcript, UserInput};

#[cfg(feature = "portable-pty")]
mod pty {
    use anyhow::Context;

    use std::str::FromStr;

    #[cfg(feature = "portable-pty")]
    #[derive(Debug, Clone, Copy)]
    pub(super) struct PtySize {
        pub rows: u16,
        pub cols: u16,
    }

    impl FromStr for PtySize {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let parts: Vec<_> = s.splitn(2, 'x').collect();
            match parts.as_slice() {
                [rows_str, cols_str] => {
                    let rows: u16 = rows_str
                        .parse()
                        .context("Cannot parse row count in PTY config")?;
                    let cols: u16 = cols_str
                        .parse()
                        .context("Cannot parse column count in PTY config")?;
                    Ok(Self { rows, cols })
                }
                _ => Err(anyhow::anyhow!(
                    "Invalid PTY config, expected a `{{rows}}x{{cols}}` string"
                )),
            }
        }
    }
}

#[cfg(feature = "portable-pty")]
use self::pty::PtySize;

#[derive(Debug, Clone, Copy)]
enum ExitCodeCheck {
    Sh,
    PowerShell,
}

impl ExitCodeCheck {
    fn for_default_shell() -> Option<Self> {
        if cfg!(unix) {
            Some(Self::Sh)
        } else {
            None
        }
    }

    fn detect(shell_command: &OsString) -> Option<Self> {
        if shell_command == "sh" || shell_command == "bash" {
            Some(Self::Sh)
        } else if shell_command == "powershell" || shell_command == "pwsh" {
            Some(Self::PowerShell)
        } else {
            None
        }
    }

    fn check_exit_code(self, response: &str) -> Option<ExitStatus> {
        match self {
            Self::Sh => response.trim().parse().ok().map(ExitStatus),
            Self::PowerShell => match response.trim() {
                "True" => Some(ExitStatus(0)),
                "False" => Some(ExitStatus(1)),
                _ => None,
            },
        }
    }
}

#[derive(Debug, StructOpt)]
pub(crate) struct ShellArgs {
    /// Execute shell in a pseudo-terminal (PTY), rather than connecting to it via pipes.
    /// PTY size can be specified by providing row and column count in a string like 19x80.
    #[cfg(feature = "portable-pty")]
    #[structopt(long)]
    pty: Option<Option<PtySize>>,

    /// Shell command without args (they are supplied separately). If omitted,
    /// will be set to the default OS shell (`sh` for *NIX, `cmd` for Windows).
    #[structopt(long, short = "s")]
    shell: Option<OsString>,

    /// Is the shell echoing (i.e., echoes all inputs to the output)? By default,
    /// shells are considered to be non-echoing.
    #[structopt(long)]
    echoing: bool,

    /// Arguments to supply to the shell command.
    #[structopt(name = "args", long, short = "a")]
    shell_args: Vec<OsString>,

    /// Timeout for I/O operations in milliseconds.
    #[structopt(
        name = "io-timeout",
        long,
        short = "T",
        value_name = "millis",
        default_value = "1000"
    )]
    io_timeout: u64,
}

impl ShellArgs {
    pub fn into_std_options(self) -> ShellOptions<Echoing<Command>> {
        let (options, exit_code_check) = if let Some(shell) = self.shell {
            let exit_code_check = ExitCodeCheck::detect(&shell);
            let mut command = Command::new(shell);
            command.args(self.shell_args);
            (ShellOptions::from(command), exit_code_check)
        } else {
            (ShellOptions::default(), ExitCodeCheck::for_default_shell())
        };

        let is_echoing = self.echoing || matches!(exit_code_check, Some(ExitCodeCheck::PowerShell));
        let mut options = options.echoing(is_echoing);
        if let Some(check) = exit_code_check {
            options = options.with_status_check("echo $?", move |code| check.check_exit_code(code));
        }
        options.with_io_timeout(Duration::from_millis(self.io_timeout))
    }

    #[cfg(feature = "portable-pty")]
    fn into_pty_options(self, pty_size: Option<PtySize>) -> ShellOptions<PtyCommand> {
        let (mut command, exit_code_check) = if let Some(shell) = self.shell {
            let exit_code_check = ExitCodeCheck::detect(&shell);
            let mut command = PtyCommand::new(shell);
            for arg in self.shell_args {
                command.arg(arg);
            }
            (command, exit_code_check)
        } else {
            (PtyCommand::default(), ExitCodeCheck::for_default_shell())
        };

        if let Some(size) = pty_size {
            command.with_size(size.rows, size.cols);
        }

        let options =
            ShellOptions::new(command).with_io_timeout(Duration::from_millis(self.io_timeout));
        if let Some(check) = exit_code_check {
            options.with_status_check("echo $?", move |code| check.check_exit_code(code))
        } else {
            options
        }
    }

    #[cfg(feature = "portable-pty")]
    pub fn create_transcript(
        self,
        inputs: impl IntoIterator<Item = UserInput>,
    ) -> io::Result<Transcript> {
        if let Some(pty_size) = self.pty {
            let mut options = self.into_pty_options(pty_size);
            Transcript::from_inputs(&mut options, inputs)
        } else {
            let mut options = self.into_std_options();
            Transcript::from_inputs(&mut options, inputs)
        }
    }

    #[cfg(not(feature = "portable-pty"))]
    pub fn create_transcript(
        self,
        inputs: impl IntoIterator<Item = UserInput>,
    ) -> io::Result<Transcript> {
        let mut options = self.into_std_options();
        Transcript::from_inputs(&mut options, inputs)
    }
}
