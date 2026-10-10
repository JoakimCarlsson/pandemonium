//! Local and remote pseudoterminals with owned byte pipes.

use crate::Host;
use crate::remote::Connection;
use std::io::{self, Read, Write};
use std::sync::Arc;

pub use portable_pty::{CommandBuilder, ExitStatus};

/// A pseudoterminal with independently owned control and streams.
pub struct Pty {
    /// Terminal resize control.
    pub control: PtyControl,
    /// Process lifetime control.
    pub child: PtyChild,
    /// The terminal output.
    pub reader: Box<dyn Read + Send>,
    /// The terminal input.
    pub writer: Box<dyn Write + Send>,
}

/// Native terminal control or a channel on a transport.
pub enum PtyControl {
    /// A local master terminal.
    Local(Box<dyn portable_pty::MasterPty + Send>),
    /// A remote terminal control endpoint.
    Remote {
        /// The transport owning this terminal.
        connection: Arc<Connection>,
        /// The terminal's channel.
        channel: u32,
    },
}
impl PtyControl {
    /// Resizes the terminal on its machine.
    pub fn resize(&self, cols: usize, rows: usize) -> io::Result<()> {
        match self {
            Self::Local(master) => master.resize(size(cols, rows)).map_err(io::Error::other),
            Self::Remote {
                connection,
                channel,
            } => connection
                .request(
                    "resize",
                    serde_json::json!({"channel":channel,"cols":cols,"rows":rows}),
                )
                .map(drop),
        }
    }
}

/// Native child control or a channel on a transport.
pub enum PtyChild {
    /// The native terminal child.
    Local(Box<dyn portable_pty::Child + Send + Sync>),
    /// A remote terminal child.
    Remote {
        /// The transport owning this child.
        connection: Arc<Connection>,
        /// The terminal channel.
        channel: u32,
    },
}
impl PtyChild {
    /// Reads exit status without waiting for termination.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self {
            Self::Local(child) => child.try_wait(),
            Self::Remote {
                connection,
                channel,
            } => {
                let status: Option<(u32, Option<String>)> = serde_json::from_value(
                    connection.request("pty_status", serde_json::json!({"channel":channel}))?,
                )?;
                Ok(status.map(|(code, signal)| {
                    signal.map_or_else(
                        || ExitStatus::with_exit_code(code),
                        |signal| ExitStatus::with_signal(&signal),
                    )
                }))
            }
        }
    }
    /// Waits for the terminal child to exit.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    /// Terminates the terminal child.
    pub fn kill(&mut self) -> io::Result<()> {
        match self {
            Self::Local(child) => child.kill(),
            Self::Remote {
                connection,
                channel,
            } => connection
                .request("kill", serde_json::json!({"channel":channel}))
                .map(drop),
        }
    }
}

impl Host {
    /// Opens a terminal on this machine in `cwd`.
    pub fn pty(
        &self,
        mut command: CommandBuilder,
        cwd: &std::path::Path,
        cols: usize,
        rows: usize,
    ) -> io::Result<Pty> {
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        if let Some(remote) = &self.0 {
            return remote.pty(command, cols, rows);
        }
        let pair = portable_pty::native_pty_system()
            .openpty(size(cols, rows))
            .map_err(io::Error::other)?;
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(io::Error::other)?;
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(io::Error::other)?;
        let writer = pair.master.take_writer().map_err(io::Error::other)?;
        Ok(Pty {
            control: PtyControl::Local(pair.master),
            child: PtyChild::Local(child),
            reader,
            writer,
        })
    }
    /// The user's interactive shell on this machine.
    pub fn shell(&self) -> CommandBuilder {
        if !self.is_local() {
            return CommandBuilder::new(
                self.environment("SHELL")
                    .filter(|shell| !shell.is_empty())
                    .unwrap_or_else(|| {
                        if self.os() == "windows" {
                            "cmd.exe".to_owned()
                        } else {
                            "/bin/sh".to_owned()
                        }
                    }),
            );
        }
        CommandBuilder::new(
            std::env::var("SHELL")
                .ok()
                .filter(|shell| !shell.is_empty())
                .unwrap_or_else(|| if cfg!(windows) { "cmd.exe" } else { "/bin/sh" }.to_owned()),
        )
    }
}

/// The platform terminal dimensions, without pixel claims.
fn size(cols: usize, rows: usize) -> portable_pty::PtySize {
    portable_pty::PtySize {
        rows: rows.clamp(1, u16::MAX as usize) as u16,
        cols: cols.clamp(1, u16::MAX as usize) as u16,
        pixel_width: 0,
        pixel_height: 0,
    }
}

impl Drop for PtyChild {
    /// Releases the endpoint terminal when its final child owner is gone.
    fn drop(&mut self) {
        if let Self::Remote {
            connection,
            channel,
        } = self
        {
            let _ = connection.request("release_pty", serde_json::json!({"channel":channel}));
        }
    }
}
