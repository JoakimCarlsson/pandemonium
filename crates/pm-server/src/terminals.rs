//! Shells the server runs in pseudo-terminals, kept going when no client is attached.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use pm_protocol::{ServerMessage, TerminalInfo};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::core::{Connection, Core};

/// How many bytes of a shell's output are kept to show a client that attaches late.
const KEPT: usize = 256 * 1024;

/// How many bytes are read from a shell at a time.
const CHUNK: usize = 8 * 1024;

/// What a shell printed that is still kept, and how much was printed before it.
#[derive(Default)]
struct Output {
    /// How many bytes were printed before the first of `bytes`.
    start: u64,
    /// The latest output.
    bytes: Vec<u8>,
    /// The connections told about new output.
    watchers: Vec<Arc<Connection>>,
}

/// One shell in a pseudo-terminal.
pub(crate) struct Terminal {
    /// The folder it was started in.
    directory: String,
    /// The size of its window, as columns and rows.
    size: Mutex<(u16, u16)>,
    /// The master side of the pseudo-terminal, which resizes it.
    master: Mutex<Box<dyn MasterPty + Send>>,
    /// Where bytes typed by the reader go.
    writer: Mutex<Box<dyn Write + Send>>,
    /// The shell's process.
    child: Mutex<Box<dyn Child + Send + Sync>>,
    /// What it printed.
    output: Mutex<Output>,
}

/// The size a pseudo-terminal of `columns` and `rows` is opened at.
fn size_of(columns: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.max(1),
        cols: columns.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

impl Terminal {
    /// Starts the reader's shell in `directory`, called `label` to clients, `columns` wide and `rows` tall.
    fn start(directory: &Path, label: &str, columns: u16, rows: u16) -> Result<Self, String> {
        let pair = native_pty_system()
            .openpty(size_of(columns, rows))
            .map_err(|error| error.to_string())?;
        let mut command = CommandBuilder::new_default_prog();
        command.cwd(directory);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| error.to_string())?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            directory: label.to_owned(),
            size: Mutex::new((columns, rows)),
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            child: Mutex::new(child),
            output: Mutex::new(Output::default()),
        })
    }

    /// What the server tells a client about this shell, numbered `terminal`.
    fn info(&self, terminal: u64) -> TerminalInfo {
        let (columns, rows) = self.size.lock().map_or((80, 24), |size| *size);
        TerminalInfo {
            terminal,
            directory: self.directory.clone(),
            columns,
            rows,
        }
    }

    /// Adds what the shell printed to what is kept and tells the connections watching.
    fn print(&self, terminal: u64, data: &[u8]) {
        let Ok(mut output) = self.output.lock() else {
            return;
        };
        let offset = output.start + output.bytes.len() as u64;
        output.bytes.extend_from_slice(data);
        if output.bytes.len() > KEPT {
            let excess = output.bytes.len() - KEPT;
            output.bytes.drain(..excess);
            output.start += excess as u64;
        }
        output.watchers.retain(|connection| {
            connection
                .outbox
                .send(ServerMessage::TerminalOutput {
                    terminal,
                    offset,
                    data: data.to_vec(),
                })
                .is_ok()
        });
    }

    /// Subscribes `connection` and sends it what was printed after the byte `after`.
    fn attach(&self, terminal: u64, connection: &Arc<Connection>, after: u64) {
        let Ok(mut output) = self.output.lock() else {
            return;
        };
        let from = after.max(output.start);
        let skipped = (from - output.start) as usize;
        if skipped < output.bytes.len() {
            let _ = connection.outbox.send(ServerMessage::TerminalOutput {
                terminal,
                offset: from,
                data: output.bytes[skipped..].to_vec(),
            });
        }
        if !output
            .watchers
            .iter()
            .any(|held| Arc::ptr_eq(held, connection))
        {
            output.watchers.push(connection.clone());
        }
    }

    /// Hands the shell what the reader typed.
    fn write(&self, data: &[u8]) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(data);
            let _ = writer.flush();
        }
    }

    /// Resizes the shell's window.
    fn resize(&self, columns: u16, rows: u16) {
        if let Ok(master) = self.master.lock() {
            let _ = master.resize(size_of(columns, rows));
        }
        if let Ok(mut size) = self.size.lock() {
            *size = (columns, rows);
        }
    }

    /// Ends the shell's process.
    fn kill(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

impl Core {
    /// Starts a shell in `directory`, which clients know by `label`, and returns its number.
    pub(crate) fn open_terminal(
        self: &Arc<Self>,
        directory: &Path,
        label: &str,
        columns: u16,
        rows: u16,
    ) -> Result<u64, String> {
        let terminal = Arc::new(Terminal::start(directory, label, columns, rows)?);
        let mut reader = terminal
            .master
            .lock()
            .map_err(|_| "the server is shutting down".to_owned())?
            .try_clone_reader()
            .map_err(|error| error.to_string())?;
        let number = self.next_terminal();
        if let Ok(mut held) = self.terminals.lock() {
            held.insert(number, terminal.clone());
        }
        let core = self.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; CHUNK];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                terminal.print(number, &buffer[..count]);
            }
            if let Ok(mut held) = core.terminals.lock() {
                held.remove(&number);
            }
            core.broadcast(&ServerMessage::TerminalExited { terminal: number }, None);
        });
        Ok(number)
    }

    /// The shell numbered `number`, if the server still holds it.
    fn terminal(&self, number: u64) -> Option<Arc<Terminal>> {
        self.terminals.lock().ok()?.get(&number).cloned()
    }

    /// Subscribes `connection` to the shell numbered `number`; false when there is none.
    pub(crate) fn attach_terminal(
        &self,
        connection: &Arc<Connection>,
        number: u64,
        after: u64,
    ) -> bool {
        match self.terminal(number) {
            Some(terminal) => {
                terminal.attach(number, connection, after);
                true
            }
            None => false,
        }
    }

    /// Types `data` into the shell numbered `number`; false when there is none.
    pub(crate) fn type_into_terminal(&self, number: u64, data: &[u8]) -> bool {
        self.terminal(number)
            .map(|terminal| terminal.write(data))
            .is_some()
    }

    /// Resizes the window of the shell numbered `number`; false when there is none.
    pub(crate) fn resize_terminal(&self, number: u64, columns: u16, rows: u16) -> bool {
        self.terminal(number)
            .map(|terminal| terminal.resize(columns, rows))
            .is_some()
    }

    /// Ends the shell numbered `number` and forgets it; false when there is none.
    pub(crate) fn close_terminal(&self, number: u64) -> bool {
        let removed = self
            .terminals
            .lock()
            .ok()
            .and_then(|mut held| held.remove(&number));
        match removed {
            Some(terminal) => {
                terminal.kill();
                true
            }
            None => false,
        }
    }

    /// What the server knows about each shell, oldest first.
    pub(crate) fn terminal_infos(&self) -> Vec<TerminalInfo> {
        let Ok(held) = self.terminals.lock() else {
            return Vec::new();
        };
        let mut infos: Vec<TerminalInfo> = held
            .iter()
            .map(|(number, terminal)| terminal.info(*number))
            .collect();
        infos.sort_by_key(|info| info.terminal);
        infos
    }
}
