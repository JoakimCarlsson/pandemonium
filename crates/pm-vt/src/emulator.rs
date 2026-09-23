//! The escape-sequence interpreter: bytes in, changes to the grid out.
//!
//! Everything a program can say to a terminal arrives here as one of the
//! parser's callbacks and leaves as a change to [`Grid`], a change to
//! [`Modes`], or a reply the terminal owes the program. The emulator holds
//! both screens, because which one is showing is itself a mode.

use unicode_width::UnicodeWidthChar;
use vte::{Params, Perform};

use crate::grid::Grid;
use crate::link::Links;
use crate::modes::Modes;
use crate::sgr;

/// How many lines of scrollback the main screen keeps.
const SCROLLBACK: usize = 10_000;

/// Both screens, the modes in force and the replies the program is owed.
pub struct Emulator {
    /// The main screen, the one with scrollback behind it.
    primary: Grid,
    /// The screen a full-screen program draws on instead.
    alternate: Grid,
    /// Whether the alternate screen is the one showing.
    on_alternate: bool,
    /// The modes the current program has set.
    modes: Modes,
    /// The title the program has given the terminal.
    title: String,
    /// Bytes the terminal owes the program, to be written back to the pty.
    replies: Vec<u8>,
    /// Whether G0 is the DEC line-drawing set rather than ASCII.
    line_drawing: bool,
    /// Every target the program has marked a link as going to.
    links: Links,
}

impl Emulator {
    /// An emulator drawing on a blank screen of `cols` by `rows`.
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            primary: Grid::new(cols, rows, SCROLLBACK),
            alternate: Grid::new(cols, rows, 0),
            on_alternate: false,
            modes: Modes::DEFAULT,
            title: String::new(),
            replies: Vec::new(),
            line_drawing: false,
            links: Links::default(),
        }
    }

    /// The screen that is showing.
    pub fn grid(&self) -> &Grid {
        if self.on_alternate {
            &self.alternate
        } else {
            &self.primary
        }
    }

    /// The screen that is showing, to be written to.
    pub fn grid_mut(&mut self) -> &mut Grid {
        if self.on_alternate {
            &mut self.alternate
        } else {
            &mut self.primary
        }
    }

    /// The modes the current program has set.
    pub fn modes(&self) -> Modes {
        self.modes
    }

    /// The title the program has given the terminal.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Every target the program has marked a link as going to.
    pub fn links(&self) -> &Links {
        &self.links
    }

    /// Takes the bytes the terminal owes the program.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    /// Resizes both screens to `cols` by `rows`.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.primary.resize(cols, rows);
        self.alternate.resize(cols, rows);
    }

    /// Shows the alternate screen, or goes back to the main one.
    fn set_alternate(&mut self, active: bool) {
        if active == self.on_alternate {
            return;
        }
        if active {
            self.primary.save_cursor();
            self.alternate.erase_in_display(2);
            self.alternate.goto(0, 0);
        } else {
            self.primary.restore_cursor();
        }
        self.on_alternate = active;
        self.grid_mut().scroll_to_bottom();
    }

    /// Turns one mode on or off, whether private to DEC or not.
    fn set_mode(&mut self, mode: u16, private: bool, on: bool) {
        if !private {
            if mode == 4 {
                self.modes.insert = on;
            }
            return;
        }
        match mode {
            1 => self.modes.application_cursor = on,
            7 => self.modes.wrap = on,
            25 => self.modes.cursor_visible = on,
            47 | 1047 => self.set_alternate(on),
            1048 => {
                if on {
                    self.grid_mut().save_cursor();
                } else {
                    self.grid_mut().restore_cursor();
                }
            }
            1049 => {
                if on {
                    self.grid_mut().save_cursor();
                }
                self.set_alternate(on);
            }
            1004 => self.modes.focus_reporting = on,
            2004 => self.modes.bracketed_paste = on,
            _ => {}
        }
    }

    /// Returns the terminal to the state it starts a program in.
    fn reset(&mut self) {
        let (cols, rows) = (self.primary.cols(), self.primary.rows());
        self.primary = Grid::new(cols, rows, SCROLLBACK);
        self.alternate = Grid::new(cols, rows, 0);
        self.on_alternate = false;
        self.modes = Modes::DEFAULT;
        self.line_drawing = false;
    }

    /// Opens the link OSC 8 names for what is written next, or closes it.
    ///
    /// The parser splits the command at every semicolon, and a target may
    /// hold semicolons of its own, so everything after the parameters is
    /// joined back into the one target it was sent as. An empty target is
    /// how a program says the link is over.
    fn mark_link(&mut self, target: &[&[u8]]) {
        let target = target
            .iter()
            .map(|part| String::from_utf8_lossy(part))
            .collect::<Vec<_>>()
            .join(";");
        let link = (!target.is_empty()).then(|| self.links.intern(&target));
        self.primary.set_link(link);
        self.alternate.set_link(link);
    }

    /// Answers a Device Status Report.
    fn report_status(&mut self, request: u16) {
        match request {
            5 => self.replies.extend_from_slice(b"\x1b[0n"),
            6 => {
                let cursor = self.grid().cursor();
                let report = format!("\x1b[{};{}R", cursor.row + 1, cursor.col + 1);
                self.replies.extend_from_slice(report.as_bytes());
            }
            _ => {}
        }
    }
}

impl Perform for Emulator {
    /// Writes one character at the cursor.
    fn print(&mut self, c: char) {
        let c = if self.line_drawing { line_drawn(c) } else { c };
        let width = c.width().unwrap_or(1);
        if width == 0 {
            return;
        }
        let (wrap, insert) = (self.modes.wrap, self.modes.insert);
        let grid = self.grid_mut();
        grid.scroll_to_bottom();
        grid.write(c, width, wrap, insert);
    }

    /// Carries out one C0 control character.
    fn execute(&mut self, byte: u8) {
        let grid = self.grid_mut();
        match byte {
            0x08 => grid.backspace(),
            0x09 => grid.tab(1),
            0x0a..=0x0c => {
                grid.scroll_to_bottom();
                grid.index();
            }
            0x0d => grid.carriage_return(),
            _ => {}
        }
    }

    /// Carries out one `CSI` sequence.
    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let private = intermediates.first() == Some(&b'?');
        let arg = |index: usize| -> u16 {
            params
                .iter()
                .nth(index)
                .and_then(|values| values.first().copied())
                .filter(|value| *value != 0)
                .unwrap_or(1)
        };
        let raw = |index: usize| -> u16 {
            params
                .iter()
                .nth(index)
                .and_then(|values| values.first().copied())
                .unwrap_or(0)
        };
        let count = arg(0) as usize;

        match action {
            'm' => {
                let mut attrs = self.grid().attrs();
                sgr::apply(&mut attrs, params);
                self.grid_mut().set_attrs(attrs);
            }
            'h' | 'l' => {
                let on = action == 'h';
                for mode in params.iter().filter_map(|values| values.first().copied()) {
                    self.set_mode(mode, private, on);
                }
            }
            'r' => {
                let bottom = match raw(1) {
                    0 => self.grid().rows(),
                    value => value as usize,
                };
                let top = arg(0) as usize;
                self.grid_mut()
                    .set_region(top - 1, bottom.saturating_sub(1));
            }
            'n' => self.report_status(raw(0)),
            'c' => self.replies.extend_from_slice(b"\x1b[?6c"),
            '@' => self.grid_mut().insert_chars(count),
            'A' => self.grid_mut().move_up(count),
            'B' | 'e' => self.grid_mut().move_down(count),
            'C' | 'a' => self.grid_mut().move_right(count),
            'D' => self.grid_mut().move_left(count),
            'E' => {
                let grid = self.grid_mut();
                grid.move_down(count);
                grid.carriage_return();
            }
            'F' => {
                let grid = self.grid_mut();
                grid.move_up(count);
                grid.carriage_return();
            }
            'G' | '`' => {
                let row = self.grid().cursor().row;
                self.grid_mut().goto(row, count - 1);
            }
            'd' => {
                let col = self.grid().cursor().col;
                self.grid_mut().goto(count - 1, col);
            }
            'H' | 'f' => {
                let (row, col) = (arg(0) as usize, arg(1) as usize);
                self.grid_mut().goto(row - 1, col - 1);
            }
            'I' => self.grid_mut().tab(count),
            'J' => self.grid_mut().erase_in_display(raw(0)),
            'K' => self.grid_mut().erase_in_line(raw(0)),
            'L' => self.grid_mut().insert_lines(count),
            'M' => self.grid_mut().delete_lines(count),
            'P' => self.grid_mut().delete_chars(count),
            'S' => self.grid_mut().scroll_up(count),
            'T' => self.grid_mut().scroll_down(count),
            'X' => self.grid_mut().erase_chars(count),
            'g' => {
                let all = raw(0) == 3;
                self.grid_mut().clear_tabs(all);
            }
            's' => self.grid_mut().save_cursor(),
            'u' => self.grid_mut().restore_cursor(),
            _ => {}
        }
    }

    /// Carries out one `ESC` sequence that is not a `CSI` one.
    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match (intermediates.first(), byte) {
            (Some(b'('), b'0') => self.line_drawing = true,
            (Some(b'('), _) => self.line_drawing = false,
            (None, b'D') => self.grid_mut().index(),
            (None, b'E') => {
                let grid = self.grid_mut();
                grid.index();
                grid.carriage_return();
            }
            (None, b'H') => self.grid_mut().set_tab(),
            (None, b'M') => self.grid_mut().reverse_index(),
            (None, b'7') => self.grid_mut().save_cursor(),
            (None, b'8') => self.grid_mut().restore_cursor(),
            (None, b'=') => self.modes.application_keypad = true,
            (None, b'>') => self.modes.application_keypad = false,
            (None, b'c') => self.reset(),
            _ => {}
        }
    }

    /// Carries out one operating-system command: the title, or a link.
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let Some(kind) = params.first() else {
            return;
        };
        match *kind {
            b"0" | b"2" => {
                if let Some(title) = params.get(1) {
                    self.title = String::from_utf8_lossy(title).into_owned();
                }
            }
            b"8" => self.mark_link(params.get(2..).unwrap_or_default()),
            _ => {}
        }
    }
}

/// The DEC line-drawing glyph `c` stands for while that set is selected.
///
/// The set is how a program from before Unicode draws a box, and it is still
/// what `ncurses` reaches for, so a terminal without it draws `lqqqk` where a
/// border belongs.
fn line_drawn(c: char) -> char {
    match c {
        '`' => '◆',
        'a' => '▒',
        'f' => '°',
        'g' => '±',
        'i' => '␋',
        'j' => '┘',
        'k' => '┐',
        'l' => '┌',
        'm' => '└',
        'n' => '┼',
        'o' => '⎺',
        'p' => '⎻',
        'q' => '─',
        'r' => '⎼',
        's' => '⎽',
        't' => '├',
        'u' => '┤',
        'v' => '┴',
        'w' => '┬',
        'x' => '│',
        'y' => '≤',
        'z' => '≥',
        '{' => 'π',
        '|' => '≠',
        '}' => '£',
        '~' => '·',
        other => other,
    }
}
