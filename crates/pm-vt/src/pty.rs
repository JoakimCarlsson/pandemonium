//! The pseudoterminal a shell or an agent CLI runs in.
//!
//! The pty is the only part of the crate that owns a thread. Reading a pty
//! blocks, so a reader thread does it and hands whole chunks over a channel;
//! the caller pumps the channel whenever it is woken, and `notify` is what
//! wakes it.

use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{Child, CommandBuilder, ExitStatus, MasterPty, PtySize, native_pty_system};

/// How many bytes the reader thread hands over at a time.
const CHUNK: usize = 8192;

/// How long the reader thread waits between looks at a child whose pty has
/// closed, for the child to be done exiting.
const REAP_PAUSE: Duration = Duration::from_millis(20);

/// How many looks it takes before it stops waiting.
///
/// A child that closes its pty and goes on running is a daemon, not one that
/// is exiting slowly, and the caller learns of it the way it always would.
const REAP_LOOKS: usize = 100;

/// Told to the caller whenever the child has written something.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// A child process, the pty it runs in and the bytes it has written.
pub struct Pty {
    /// The master side, which is what a resize is applied to.
    master: Box<dyn MasterPty + Send>,
    /// The master side's input, which is what a keypress is written to.
    writer: Box<dyn Write + Send>,
    /// Chunks the reader thread has read, oldest first.
    output: Receiver<Vec<u8>>,
    /// The process itself, so it can be waited on and killed, shared with
    /// the reader thread so that it can say when the child has exited.
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    /// Whether the child's end of the pty has closed.
    closed: bool,
}

impl Pty {
    /// Starts `command` in a pty of `cols` by `rows`, rooted at `cwd`.
    pub fn spawn(
        command: CommandBuilder,
        cwd: &Path,
        cols: usize,
        rows: usize,
        notify: Notify,
    ) -> std::io::Result<Self> {
        let pair = native_pty_system()
            .openpty(size(cols, rows))
            .map_err(std::io::Error::other)?;

        let mut command = command;
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");

        let child = Arc::new(Mutex::new(
            pair.slave
                .spawn_command(command)
                .map_err(std::io::Error::other)?,
        ));
        let reaped = child.clone();
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(std::io::Error::other)?;
        let writer = pair.master.take_writer().map_err(std::io::Error::other)?;
        let (sender, output) = channel();

        std::thread::Builder::new()
            .name("pm-vt-pty".to_owned())
            .spawn(move || {
                let mut buffer = [0u8; CHUNK];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(read) => {
                            if sender.send(buffer[..read].to_vec()).is_err() {
                                break;
                            }
                            notify();
                        }
                        Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                for _ in 0..REAP_LOOKS {
                    let exited = reaped
                        .lock()
                        .map_or(true, |mut child| !matches!(child.try_wait(), Ok(None)));
                    if exited {
                        break;
                    }
                    std::thread::sleep(REAP_PAUSE);
                }
                notify();
            })?;

        Ok(Self {
            master: pair.master,
            writer,
            output,
            child,
            closed: false,
        })
    }

    /// The default interactive shell, as the platform and the user set it.
    pub fn shell() -> CommandBuilder {
        #[cfg(windows)]
        let fallback = "cmd.exe".to_owned();
        #[cfg(not(windows))]
        let fallback = "/bin/sh".to_owned();

        let program = std::env::var("SHELL")
            .ok()
            .filter(|shell| !shell.is_empty())
            .unwrap_or(fallback);
        CommandBuilder::new(program)
    }

    /// Takes everything the child has written since the last call.
    pub fn read(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        loop {
            match self.output.try_recv() {
                Ok(chunk) => bytes.extend_from_slice(&chunk),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    break;
                }
            }
        }
        bytes
    }

    /// Writes `bytes` to the child's input.
    pub fn write(&mut self, bytes: &[u8]) {
        if self.writer.write_all(bytes).is_err() {
            self.closed = true;
            return;
        }
        let _ = self.writer.flush();
    }

    /// Tells the child the window is now `cols` by `rows`.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let _ = self.master.resize(size(cols, rows));
    }

    /// Whether the child is still running.
    pub fn is_running(&mut self) -> bool {
        !self.closed && matches!(self.status(), Ok(None))
    }

    /// Ends the child, if it is still running.
    pub fn kill(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }

    /// The code the child exited with, once it has exited.
    pub fn exit_code(&mut self) -> Option<u32> {
        self.status()
            .ok()
            .flatten()
            .map(|status| status.exit_code())
    }

    /// The signal that ended the child, once a signal has.
    pub fn exit_signal(&mut self) -> Option<String> {
        self.status()
            .ok()
            .flatten()
            .and_then(|status| status.signal().map(str::to_owned))
    }

    /// How the child exited, once it has.
    fn status(&self) -> std::io::Result<Option<ExitStatus>> {
        self.child
            .lock()
            .map_err(|_| std::io::Error::other("the child's lock was poisoned"))?
            .try_wait()
    }
}

impl Drop for Pty {
    /// Kills the child, so closing a pane does not leave a shell behind.
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The pty size for a grid of `cols` by `rows`.
///
/// The pixel dimensions stay zero: nothing the editor runs asks for them, and
/// a wrong answer is worse than no answer.
fn size(cols: usize, rows: usize) -> PtySize {
    PtySize {
        rows: rows.max(1) as u16,
        cols: cols.max(1) as u16,
        pixel_width: 0,
        pixel_height: 0,
    }
}
