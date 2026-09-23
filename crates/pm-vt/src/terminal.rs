//! One terminal: a child in a pty, the parser reading it and the grid it writes.
//!
//! This is the seam everything else uses. A pane holds a [`Terminal`], pumps
//! it when it is woken, draws its [`Grid`] and hands it keypresses; it never
//! sees the pty, the parser or the emulator behind them.

use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;
use vte::Parser;

use crate::emulator::Emulator;
use crate::grid::Grid;
use crate::keys::{self, Key, Modifiers};
use crate::link::{self, Link};
use crate::modes::Modes;
use crate::pty::{Notify, Pty};
use crate::selection::{Place, Selection, Unit};

/// A child in a pty and the screen it is drawing on.
pub struct Terminal {
    /// The escape-sequence state machine.
    parser: Parser,
    /// What the sequences are applied to.
    emulator: Emulator,
    /// The child and the pty it runs in.
    pty: Pty,
    /// Columns the child has been told about.
    cols: usize,
    /// Rows the child has been told about.
    rows: usize,
    /// The directory the child was started in.
    cwd: PathBuf,
    /// The program the child is running.
    program: String,
}

impl Terminal {
    /// Starts the user's shell in `cwd` on a screen of `cols` by `rows`.
    ///
    /// The `env` is what the worktree adds to the one the editor was started
    /// with — a session's own port, and whatever else is its rather than the
    /// machine's.
    pub fn shell(
        cwd: impl Into<PathBuf>,
        cols: usize,
        rows: usize,
        env: &[(String, String)],
        notify: Notify,
    ) -> std::io::Result<Self> {
        let mut command = Pty::shell();
        for (name, value) in env {
            command.env(name, value);
        }
        Self::spawn(command, cwd, cols, rows, notify)
    }

    /// Starts `command` in `cwd` on a screen of `cols` by `rows`.
    pub fn spawn(
        command: CommandBuilder,
        cwd: impl Into<PathBuf>,
        cols: usize,
        rows: usize,
        notify: Notify,
    ) -> std::io::Result<Self> {
        let cwd = cwd.into();
        let (cols, rows) = (cols.max(1), rows.max(1));
        let program = command
            .get_argv()
            .first()
            .map(|program| program.to_string_lossy().into_owned())
            .unwrap_or_default();
        let pty = Pty::spawn(command, &cwd, cols, rows, notify)?;

        Ok(Self {
            parser: Parser::new(),
            emulator: Emulator::new(cols, rows),
            pty,
            cols,
            rows,
            cwd,
            program,
        })
    }

    /// Applies everything the child has written, and answers what it asked.
    ///
    /// The answer goes back through the same pty the question came from: a
    /// status report or a device attributes request is the terminal's own
    /// reply, not something a caller should have to forward.
    pub fn pump(&mut self) -> bool {
        let bytes = self.pty.read();
        if bytes.is_empty() {
            return false;
        }
        self.parser.advance(&mut self.emulator, &bytes);

        let replies = self.emulator.take_replies();
        if !replies.is_empty() {
            self.pty.write(&replies);
        }
        true
    }

    /// Tells the child and both screens that the pane is `cols` by `rows`.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let (cols, rows) = (cols.max(1), rows.max(1));
        if cols == self.cols && rows == self.rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.emulator.resize(cols, rows);
        self.pty.resize(cols, rows);
    }

    /// Sends what `key` means to the child, and returns to the live screen.
    pub fn press(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let Some(bytes) = keys::encode(key, modifiers, self.emulator.modes()) else {
            return false;
        };
        self.send(&bytes);
        true
    }

    /// Sends `text` to the child as a paste.
    pub fn paste(&mut self, text: &str) {
        let bytes = keys::paste(text, self.emulator.modes());
        self.send(&bytes);
    }

    /// Writes `bytes` to the child, showing the live screen again.
    ///
    /// Whatever was picked out is let go of: what is sent is about to change
    /// the screen under it.
    pub fn send(&mut self, bytes: &[u8]) {
        let grid = self.emulator.grid_mut();
        grid.scroll_to_bottom();
        grid.set_selection(None);
        self.pty.write(bytes);
    }

    /// The place of the cell at `row` and `col` of the view.
    pub fn place_at(&self, row: usize, col: usize) -> Place {
        self.grid().place_at(row, col)
    }

    /// Picks out the cells from `anchor` to `head`, grown by `unit`.
    pub fn select(&mut self, anchor: Place, head: Place, unit: Unit) {
        self.emulator
            .grid_mut()
            .set_selection(Some(Selection { anchor, head, unit }));
    }

    /// Picks out everything the screen and its scrollback hold.
    pub fn select_all(&mut self) {
        let (anchor, head) = self.grid().extent();
        self.select(anchor, head, Unit::Cell);
    }

    /// Lets go of what was picked out.
    pub fn clear_selection(&mut self) {
        self.emulator.grid_mut().set_selection(None);
    }

    /// The first and last cells picked out, when anything is.
    pub fn selection_span(&self) -> Option<(Place, Place)> {
        let grid = self.grid();
        grid.selection().map(|selection| selection.span(grid))
    }

    /// The text picked out, when there is any.
    pub fn selected_text(&self) -> Option<String> {
        let grid = self.grid();
        let text = grid.selection()?.text(grid);
        (!text.is_empty()).then_some(text)
    }

    /// The link written across `place`, if one is.
    pub fn link_at(&self, place: Place) -> Option<Link> {
        link::link_at(self.grid(), self.emulator.links(), place)
    }

    /// Scrolls the view `lines` rows back through the scrollback.
    pub fn scroll(&mut self, lines: isize) {
        self.emulator.grid_mut().scroll_view(lines);
    }

    /// Puts the view `lines` rows back through the scrollback.
    pub fn scroll_to(&mut self, lines: usize) {
        self.emulator.grid_mut().scroll_to(lines);
    }

    /// Keeps `lines` of scrollback behind the main screen from now on.
    pub fn set_scrollback(&mut self, lines: usize) {
        self.emulator.set_scrollback(lines);
    }

    /// The screen that is showing.
    pub fn grid(&self) -> &Grid {
        self.emulator.grid()
    }

    /// The modes the current program has set.
    pub fn modes(&self) -> Modes {
        self.emulator.modes()
    }

    /// The title the program has given the terminal, if it has given one.
    pub fn title(&self) -> Option<&str> {
        Some(self.emulator.title()).filter(|title| !title.is_empty())
    }

    /// The directory the child was started in.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// The program the child is running.
    pub fn program(&self) -> &str {
        &self.program
    }

    /// Whether the child is still running.
    pub fn is_running(&mut self) -> bool {
        self.pty.is_running()
    }
}
