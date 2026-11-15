//! The shells the dock shows: several per folder, each kept by the server and
//! mirrored here in an emulator, and the plain screen the pages draw.

use pm_terminal::{Cell, Emulator, Reach};

/// How many characters wide a shell starts before the dock reports its size.
pub const START_COLUMNS: u16 = 120;

/// How many lines tall a shell starts before the dock reports its size.
pub const START_ROWS: u16 = 8;

/// A first and a last cell, each a line of the window and a column, in reading order.
pub type WindowSpan = ((usize, usize), (usize, usize));

/// What a terminal window shows at one moment, as plain data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalScreen {
    /// The cells of each line of the window, top first.
    pub lines: Vec<Vec<Cell>>,
    /// The line and column of the cursor, when it is drawn.
    pub cursor: Option<(usize, usize)>,
    /// The first and last marked cell in the window.
    pub selection: Option<WindowSpan>,
    /// How many lines the window is scrolled back from the end.
    pub back: usize,
    /// Whether the arrow keys send the application form.
    pub application_cursor: bool,
    /// Whether the shell ended.
    pub exited: bool,
}

impl TerminalScreen {
    /// The screen `emulator` shows, scrolled `back` lines up from the end.
    pub fn of(emulator: &Emulator, back: usize) -> Self {
        Self {
            lines: (0..emulator.rows())
                .map(|row| emulator.line(row, back).to_vec())
                .collect(),
            cursor: emulator.cursor(back),
            selection: window_selection(emulator, back),
            back,
            application_cursor: emulator.application_cursor(),
            exited: false,
        }
    }

    /// The screen of a fresh emulator that was fed `bytes`, for samples and tests.
    pub fn printed(bytes: &[u8], columns: usize, rows: usize) -> Self {
        let mut emulator = Emulator::new(columns, rows);
        emulator.feed(bytes);
        Self::of(&emulator, 0)
    }

    /// The text of each line with trailing blanks dropped.
    pub fn text(&self) -> Vec<String> {
        self.lines
            .iter()
            .map(|line| {
                line.iter()
                    .map(|cell| cell.ch)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }
}

/// The part of what `emulator` marks that the window shows when scrolled `back` lines up.
fn window_selection(emulator: &Emulator, back: usize) -> Option<WindowSpan> {
    let (start, end) = emulator.selection()?.ordered();
    let first = emulator.first_line(back);
    let rows = emulator.rows();
    if end.0 < first || start.0 >= first + rows {
        return None;
    }
    let from = if start.0 < first {
        (0, 0)
    } else {
        (start.0 - first, start.1)
    };
    let to = if end.0 >= first + rows {
        (rows - 1, emulator.columns())
    } else {
        (end.0 - first, end.1)
    };
    Some((from, to))
}

/// One shell the server runs in a folder, mirrored in an emulator.
pub struct Shell {
    /// The number the window keeps it by.
    pub key: u64,
    /// The folder it runs in.
    pub folder: String,
    /// The task it runs, when it was started for one.
    pub task: Option<String>,
    /// The server's number for it, once it answered.
    pub id: Option<u64>,
    /// What the shell printed, laid out as a terminal does.
    pub emulator: Emulator,
    /// How many lines the window is scrolled back from the end.
    pub back: usize,
    /// Whether the shell ended.
    pub exited: bool,
}

impl Shell {
    /// A shell numbered `key` asked for in `folder`, `columns` by `rows`, and not yet started.
    pub fn starting(key: u64, folder: String, columns: u16, rows: u16) -> Self {
        Self {
            key,
            folder,
            task: None,
            id: None,
            emulator: Emulator::new(columns.into(), rows.into()),
            back: 0,
            exited: false,
        }
    }

    /// What the shell shows now.
    pub fn screen(&self) -> TerminalScreen {
        TerminalScreen {
            exited: self.exited,
            ..TerminalScreen::of(&self.emulator, self.back)
        }
    }

    /// Reads `data` the shell printed, keeping a window scrolled back on the same lines as the history grows.
    pub fn feed(&mut self, data: &[u8]) {
        let before = self.emulator.history();
        self.emulator.feed(data);
        let after = self.emulator.history();
        if self.back > 0 {
            self.back = (self.back + after.saturating_sub(before)).min(after);
        }
    }

    /// Scrolls the window `lines` back, when positive, or forward, within the history.
    pub fn scroll(&mut self, lines: i32) {
        let history = self.emulator.history() as i64;
        self.back = (self.back as i64 + i64::from(lines)).clamp(0, history) as usize;
    }

    /// Marks from `anchor` to `head`, each a line of the window and a column, by cells, words or lines as `clicks` says; a single press that did not move unmarks.
    pub fn select(&mut self, anchor: (usize, usize), head: (usize, usize), clicks: u32) {
        if clicks <= 1 && anchor == head {
            self.emulator.clear_selection();
            return;
        }
        let first = self.emulator.first_line(self.back);
        let anchor = (first + anchor.0, anchor.1);
        let head = (first + head.0, head.1);
        let reach = match clicks {
            0 | 1 => Reach::Point,
            2 => Reach::Word,
            _ => Reach::Line,
        };
        let around_anchor = self.emulator.reach_around(reach, anchor);
        let around_head = self.emulator.reach_around(reach, head);
        if head >= anchor {
            self.emulator.select(around_anchor.0, around_head.1);
        } else {
            self.emulator.select(around_anchor.1, around_head.0);
        }
    }

    /// The bytes that type `text` into the shell as a paste: line ends as returns, control characters other than tab and return dropped so the paste cannot end early, wrapped in markers when the program asked for them.
    pub fn paste_bytes(&self, text: &str) -> Vec<u8> {
        let text: String = text
            .replace("\r\n", "\r")
            .replace('\n', "\r")
            .chars()
            .filter(|character| *character == '\r' || *character == '\t' || !character.is_control())
            .collect();
        if self.emulator.bracketed_paste() {
            format!("\x1b[200~{text}\x1b[201~").into_bytes()
        } else {
            text.into_bytes()
        }
    }

    /// The bytes the wheel sends a full-screen program that has no history: an arrow key for each line, up for `lines` back.
    pub fn wheel_bytes(&self, lines: i32) -> Vec<u8> {
        let letter = if lines > 0 { 'A' } else { 'B' };
        let arrow = if self.emulator.application_cursor() {
            format!("\x1bO{letter}")
        } else {
            format!("\x1b[{letter}")
        };
        arrow.repeat(lines.unsigned_abs() as usize).into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Printed text lands on the screen's lines with the cursor after it.
    #[test]
    fn printed_text_fills_the_screen() {
        let screen = TerminalScreen::printed(b"one\r\n$ ", 20, 3);
        assert_eq!(screen.text(), vec!["one", "$", ""]);
        assert_eq!(screen.cursor, Some((1, 2)));
    }

    /// A shell scrolled back keeps showing the same lines as more is printed, and stops at the oldest.
    #[test]
    fn scrolling_back_holds_its_place() {
        let mut shell = Shell::starting(1, "~/".to_owned(), 10, 2);
        shell.feed(b"a\r\nb\r\nc\r\n");
        shell.scroll(1);
        assert_eq!(shell.screen().text(), vec!["b", "c"]);
        shell.feed(b"d\r\n");
        assert_eq!(shell.screen().text(), vec!["b", "c"]);
        shell.scroll(100);
        assert_eq!(shell.screen().text()[0], "a");
    }

    /// A double press marks the word under it, and the marked text can be read back.
    #[test]
    fn double_press_marks_a_word() {
        let mut shell = Shell::starting(1, "~/".to_owned(), 20, 2);
        shell.feed(b"hello world");
        shell.select((0, 7), (0, 7), 2);
        assert_eq!(shell.emulator.selected_text().as_deref(), Some("world"));
        assert_eq!(shell.screen().selection, Some(((0, 6), (0, 10))));
    }

    /// A paste drops escape and other control characters, so it cannot leave bracketed paste mode, but keeps tabs and line ends.
    #[test]
    fn paste_strips_control_characters() {
        let mut shell = Shell::starting(1, "~/".to_owned(), 20, 2);
        assert_eq!(shell.paste_bytes("a\tb\r\nc\x1b[201~d\x07"), b"a\tb\rc[201~d".to_vec());
        shell.feed(b"\x1b[?2004h");
        assert_eq!(shell.paste_bytes("x\x1by"), b"\x1b[200~xy\x1b[201~".to_vec());
    }
}
