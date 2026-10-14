//! The emulator: the bytes a shell prints, turned into what a terminal window shows.

use vte::{Params, Parser, Perform};

use crate::cell::{Cell, Color, Flags};
use crate::grid::{Cursor, Grid};

/// How many columns an emulator starts with.
const COLUMNS: usize = 80;

/// How many lines an emulator starts with.
const ROWS: usize = 24;

/// How far apart tab stops are.
const TAB: usize = 8;

/// The characters the DEC line-drawing set puts on the keys from backtick to tilde.
const LINES: [char; 31] = [
    '◆', '▒', '␉', '␌', '␍', '␊', '°', '±', '␤', '␋', '┘', '┐', '┌', '└', '┼', '⎺', '⎻', '─', '⎼',
    '⎽', '├', '┤', '┴', '┬', '│', '≤', '≥', 'π', '≠', '£', '·',
];

/// A part of the output the reader marked, from where the press began to where the pointer is, in lines counted from the oldest kept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    /// Where it began, as a line and a column.
    pub anchor: (usize, usize),
    /// Where it ends now, as a line and a column.
    pub head: (usize, usize),
}

impl Selection {
    /// The two ends in reading order.
    pub fn ordered(&self) -> ((usize, usize), (usize, usize)) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

/// The pen: how the next characters are drawn.
#[derive(Clone, Copy, Debug, Default)]
struct Pen {
    /// The foreground.
    fg: Color,
    /// The background.
    bg: Color,
    /// How characters are drawn.
    flags: Flags,
}

/// What a character set puts on the keys.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Charset {
    /// The characters as they are.
    #[default]
    Plain,
    /// The DEC line-drawing set.
    Lines,
}

/// The modes a program running in the terminal switches on and off.
#[derive(Clone, Copy, Debug)]
struct Modes {
    /// Whether the cursor is drawn.
    cursor_visible: bool,
    /// Whether the arrow keys send the application form.
    application_cursor: bool,
    /// Whether pasted text is wrapped in markers.
    bracketed_paste: bool,
    /// Whether a character past the last column goes to the next line.
    wrap: bool,
}

impl Default for Modes {
    /// The modes a terminal starts in.
    fn default() -> Self {
        Self {
            cursor_visible: true,
            application_cursor: false,
            bracketed_paste: false,
            wrap: true,
        }
    }
}

/// Everything the emulator changes as it reads, apart from the parser that reads.
struct Screen {
    /// The grid with the history, which the shell normally draws on.
    primary: Grid,
    /// The grid full-screen programs draw on.
    alternate: Grid,
    /// Whether the alternate grid is the one showing.
    on_alternate: bool,
    /// The pen.
    pen: Pen,
    /// The modes.
    modes: Modes,
    /// The character sets, and which one is in use.
    charsets: [Charset; 2],
    /// Which of the character sets is in use.
    charset: usize,
    /// Where the tab stops are, by column.
    tabs: Vec<bool>,
    /// What the window is titled, when a program said.
    title: Option<String>,
    /// What the terminal answers to the program, to be sent back to the shell.
    replies: Vec<u8>,
    /// The last printed character, which a repeat repeats.
    last: char,
}

/// A terminal: a parser and the screen it draws on.
pub struct Emulator {
    /// Reads the bytes.
    parser: Parser,
    /// What the bytes draw on.
    screen: Screen,
    /// How many times the contents changed.
    version: u64,
    /// What the reader marked, if anything.
    selection: Option<Selection>,
}

impl Default for Emulator {
    /// An emulator of 80 columns and 24 lines.
    fn default() -> Self {
        Self::new(COLUMNS, ROWS)
    }
}

/// The tab stops of a line of `columns`: one at every eighth column.
fn tab_stops(columns: usize) -> Vec<bool> {
    (0..columns).map(|column| column % TAB == 0).collect()
}

impl Emulator {
    /// An empty terminal `columns` wide and `rows` tall.
    pub fn new(columns: usize, rows: usize) -> Self {
        let (columns, rows) = (columns.max(1), rows.max(1));
        Self {
            parser: Parser::new(),
            screen: Screen {
                primary: Grid::new(columns, rows, true),
                alternate: Grid::new(columns, rows, false),
                on_alternate: false,
                pen: Pen::default(),
                modes: Modes::default(),
                charsets: [Charset::Plain; 2],
                charset: 0,
                tabs: tab_stops(columns),
                title: None,
                replies: Vec::new(),
                last: ' ',
            },
            version: 0,
            selection: None,
        }
    }

    /// Reads `bytes` the shell printed.
    pub fn feed(&mut self, bytes: &[u8]) {
        let before = self.screen.primary.history.len();
        self.parser.advance(&mut self.screen, bytes);
        self.version += 1;
        if self.screen.primary.history.len() != before || self.screen.on_alternate {
            self.selection = None;
        }
    }

    /// Changes the window to `columns` by `rows`.
    pub fn resize(&mut self, columns: usize, rows: usize) {
        let (columns, rows) = (columns.max(1), rows.max(1));
        if columns == self.screen.primary.columns && rows == self.screen.primary.rows {
            return;
        }
        self.screen.primary.resize(columns, rows);
        self.screen.alternate.resize(columns, rows);
        self.screen.tabs = tab_stops(columns);
        self.selection = None;
        self.version += 1;
    }

    /// How many times the contents changed, so a drawing of them can be kept while it stays the same.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// How many columns the window has.
    pub fn columns(&self) -> usize {
        self.grid().columns
    }

    /// How many lines the window has.
    pub fn rows(&self) -> usize {
        self.grid().rows
    }

    /// The grid showing.
    fn grid(&self) -> &Grid {
        if self.screen.on_alternate {
            &self.screen.alternate
        } else {
            &self.screen.primary
        }
    }

    /// How many lines are kept above the window.
    pub fn history(&self) -> usize {
        if self.screen.on_alternate {
            0
        } else {
            self.screen.primary.history.len()
        }
    }

    /// The line `row` of the window when it is scrolled `back` lines up from the end, which is clamped to the history.
    pub fn line(&self, row: usize, back: usize) -> &[Cell] {
        let index = self.history().saturating_sub(back) + row;
        self.absolute(index)
    }

    /// The line `index` counted from the oldest kept, or an empty one past the end.
    fn absolute(&self, index: usize) -> &[Cell] {
        let grid = self.grid();
        let history = self.history();
        if index < history {
            &grid.history[index]
        } else {
            grid.lines
                .get(index - history)
                .map_or(&[][..], Vec::as_slice)
        }
    }

    /// The first line of the window, counted from the oldest kept, when it is scrolled `back` lines up.
    pub fn first_line(&self, back: usize) -> usize {
        self.history().saturating_sub(back)
    }

    /// The line and column of the cursor in the window, when it is drawn and in view.
    pub fn cursor(&self, back: usize) -> Option<(usize, usize)> {
        let at = self.grid().cursor;
        (self.screen.modes.cursor_visible && (back == 0 || self.screen.on_alternate))
            .then_some((at.row, at.column))
    }

    /// Whether the arrow keys send the application form.
    pub fn application_cursor(&self) -> bool {
        self.screen.modes.application_cursor
    }

    /// Whether pasted text is wrapped in markers.
    pub fn bracketed_paste(&self) -> bool {
        self.screen.modes.bracketed_paste
    }

    /// Whether a full-screen program has the window, with no history to scroll through.
    pub fn on_alternate_screen(&self) -> bool {
        self.screen.on_alternate
    }

    /// What a program titled the window, if one did.
    pub fn title(&self) -> Option<&str> {
        self.screen.title.as_deref()
    }

    /// What the terminal answers to the program, taken so that it can be sent to the shell.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.screen.replies)
    }

    /// Marks the text from `anchor` to `head`, each a line counted from the oldest kept and a column.
    pub fn select(&mut self, anchor: (usize, usize), head: (usize, usize)) {
        self.selection = Some(Selection { anchor, head });
        self.version += 1;
    }

    /// Unmarks the text.
    pub fn clear_selection(&mut self) {
        if self.selection.take().is_some() {
            self.version += 1;
        }
    }

    /// What is marked, if anything.
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// The marked text, lines joined with newlines and their trailing spaces dropped.
    pub fn selected_text(&self) -> Option<String> {
        let ((first_line, first_column), (last_line, last_column)) = self.selection?.ordered();
        if (first_line, first_column) == (last_line, last_column) {
            return None;
        }
        let mut text = String::new();
        for index in first_line..=last_line {
            let line = self.absolute(index);
            let from = if index == first_line { first_column } else { 0 };
            let to = if index == last_line {
                (last_column + 1).min(line.len())
            } else {
                line.len()
            };
            let row: String = line
                .get(from.min(to)..to)
                .unwrap_or_default()
                .iter()
                .map(|cell| cell.ch)
                .collect();
            text.push_str(row.trim_end());
            if index != last_line {
                text.push('\n');
            }
        }
        Some(text)
    }
}

impl Screen {
    /// The grid being drawn on.
    fn grid(&mut self) -> &mut Grid {
        if self.on_alternate {
            &mut self.alternate
        } else {
            &mut self.primary
        }
    }

    /// An empty cell in the pen's background, which is what erasing leaves.
    fn fill(&self) -> Cell {
        Cell {
            bg: self.pen.bg,
            ..Cell::default()
        }
    }

    /// Moves the cursor to `row` and `column`, kept inside the grid.
    fn move_to(&mut self, row: usize, column: usize) {
        let grid = self.grid();
        grid.cursor.row = row.min(grid.rows - 1);
        grid.cursor.column = column.min(grid.columns - 1);
        grid.cursor.pending_wrap = false;
    }

    /// Switches to the alternate grid, cleared, or back to the primary one.
    fn switch(&mut self, alternate: bool, save_cursor: bool) {
        if alternate == self.on_alternate {
            return;
        }
        if alternate {
            if save_cursor {
                self.primary.saved = self.primary.cursor;
            }
            self.on_alternate = true;
            let (columns, rows) = (self.alternate.columns, self.alternate.rows);
            self.alternate = Grid::new(columns, rows, false);
        } else {
            self.on_alternate = false;
            if save_cursor {
                self.primary.cursor = self.primary.saved;
            }
        }
    }

    /// The next tab stop after `column`, or the last column.
    fn next_tab(&self, column: usize) -> usize {
        (column + 1..self.tabs.len())
            .find(|candidate| self.tabs[*candidate])
            .unwrap_or(self.tabs.len() - 1)
    }

    /// The tab stop before `column`, or the first column.
    fn previous_tab(&self, column: usize) -> usize {
        (0..column)
            .rev()
            .find(|candidate| self.tabs[*candidate])
            .unwrap_or(0)
    }

    /// Resets the terminal to how it starts.
    fn reset(&mut self) {
        let (columns, rows) = (self.primary.columns, self.primary.rows);
        self.primary = Grid::new(columns, rows, true);
        self.alternate = Grid::new(columns, rows, false);
        self.on_alternate = false;
        self.pen = Pen::default();
        self.modes = Modes::default();
        self.charsets = [Charset::Plain; 2];
        self.charset = 0;
        self.tabs = tab_stops(columns);
    }

    /// Answers a query by writing `reply` back to the shell.
    fn reply(&mut self, reply: &str) {
        self.replies.extend_from_slice(reply.as_bytes());
    }

    /// Prints one character at the cursor.
    fn print_char(&mut self, character: char) {
        let character =
            if self.charsets[self.charset] == Charset::Lines && ('`'..='~').contains(&character) {
                LINES[character as usize - '`' as usize]
            } else {
                character
            };
        self.last = character;
        let cell = Cell {
            ch: character,
            fg: self.pen.fg,
            bg: self.pen.bg,
            flags: self.pen.flags,
        };
        let (wrap, fill) = (self.modes.wrap, self.fill());
        self.grid().put(cell, wrap, fill);
    }

    /// Applies the colours and flags of a select-graphic-rendition sequence to the pen.
    fn graphics(&mut self, params: &Params) {
        let mut groups = params.iter().peekable();
        if groups.peek().is_none() {
            self.pen = Pen::default();
        }
        while let Some(group) = groups.next() {
            let code = group.first().copied().unwrap_or(0);
            match code {
                0 => self.pen = Pen::default(),
                1 => self.pen.flags = self.pen.flags.with(Flags::BOLD),
                2 => self.pen.flags = self.pen.flags.with(Flags::DIM),
                3 => self.pen.flags = self.pen.flags.with(Flags::ITALIC),
                4 | 21 => {
                    self.pen.flags = if group.get(1) == Some(&0) {
                        self.pen.flags.without(Flags::UNDERLINE)
                    } else {
                        self.pen.flags.with(Flags::UNDERLINE)
                    }
                }
                7 => self.pen.flags = self.pen.flags.with(Flags::INVERSE),
                8 => self.pen.flags = self.pen.flags.with(Flags::HIDDEN),
                9 => self.pen.flags = self.pen.flags.with(Flags::STRIKE),
                22 => self.pen.flags = self.pen.flags.without(Flags::BOLD).without(Flags::DIM),
                23 => self.pen.flags = self.pen.flags.without(Flags::ITALIC),
                24 => self.pen.flags = self.pen.flags.without(Flags::UNDERLINE),
                27 => self.pen.flags = self.pen.flags.without(Flags::INVERSE),
                28 => self.pen.flags = self.pen.flags.without(Flags::HIDDEN),
                29 => self.pen.flags = self.pen.flags.without(Flags::STRIKE),
                30..=37 => self.pen.fg = Color::Indexed((code - 30) as u8),
                38 => {
                    if let Some(color) = extended_color(group, &mut groups) {
                        self.pen.fg = color;
                    }
                }
                39 => self.pen.fg = Color::Default,
                40..=47 => self.pen.bg = Color::Indexed((code - 40) as u8),
                48 => {
                    if let Some(color) = extended_color(group, &mut groups) {
                        self.pen.bg = color;
                    }
                }
                49 => self.pen.bg = Color::Default,
                90..=97 => self.pen.fg = Color::Indexed((code - 90 + 8) as u8),
                100..=107 => self.pen.bg = Color::Indexed((code - 100 + 8) as u8),
                _ => {}
            }
        }
    }

    /// Switches the modes `params` name on or off; `private` says they were asked for with a question mark.
    fn set_modes(&mut self, params: &Params, private: bool, on: bool) {
        for group in params.iter() {
            let mode = group.first().copied().unwrap_or(0);
            if !private {
                continue;
            }
            match mode {
                1 => self.modes.application_cursor = on,
                7 => self.modes.wrap = on,
                25 => self.modes.cursor_visible = on,
                47 | 1047 => self.switch(on, false),
                1049 => self.switch(on, true),
                2004 => self.modes.bracketed_paste = on,
                _ => {}
            }
        }
    }

    /// Erases part of the screen as `mode` of an erase-in-display asks.
    fn erase_display(&mut self, mode: u16) {
        let fill = self.fill();
        let grid = self.grid();
        let (row, column) = (grid.cursor.row, grid.cursor.column);
        match mode {
            0 => {
                grid.erase(row, column, grid.columns, fill);
                grid.erase_lines(row + 1, grid.rows, fill);
            }
            1 => {
                grid.erase_lines(0, row, fill);
                grid.erase(row, 0, column + 1, fill);
            }
            2 => grid.erase_lines(0, grid.rows, fill),
            3 => grid.history.clear(),
            _ => {}
        }
    }

    /// Erases part of the line as `mode` of an erase-in-line asks.
    fn erase_line(&mut self, mode: u16) {
        let fill = self.fill();
        let grid = self.grid();
        let (row, column) = (grid.cursor.row, grid.cursor.column);
        match mode {
            0 => grid.erase(row, column, grid.columns, fill),
            1 => grid.erase(row, 0, column + 1, fill),
            2 => grid.erase(row, 0, grid.columns, fill),
            _ => {}
        }
    }
}

/// The colour a `38` or `48` asks for, in the colon form held by `group` or the semicolon form that follows in `rest`.
fn extended_color<'a>(
    group: &[u16],
    rest: &mut std::iter::Peekable<impl Iterator<Item = &'a [u16]>>,
) -> Option<Color> {
    if group.len() > 1 {
        return match group.get(1)? {
            5 => Some(Color::Indexed(*group.get(2)? as u8)),
            2 if group.len() >= 5 => {
                let at = group.len() - 3;
                Some(Color::Rgb(
                    group[at] as u8,
                    group[at + 1] as u8,
                    group[at + 2] as u8,
                ))
            }
            _ => None,
        };
    }
    let mut next = || rest.next().and_then(|part| part.first().copied());
    match next()? {
        5 => Some(Color::Indexed(next()? as u8)),
        2 => Some(Color::Rgb(next()? as u8, next()? as u8, next()? as u8)),
        _ => None,
    }
}

/// The first parameter of `params`, or `default` when it is missing or zero.
fn first(params: &Params, default: usize) -> usize {
    params
        .iter()
        .next()
        .and_then(|group| group.first().copied())
        .filter(|value| *value != 0)
        .map_or(default, usize::from)
}

/// The parameter at `position`, or `default` when it is missing or zero.
fn nth(params: &Params, position: usize, default: usize) -> usize {
    params
        .iter()
        .nth(position)
        .and_then(|group| group.first().copied())
        .filter(|value| *value != 0)
        .map_or(default, usize::from)
}

impl Perform for Screen {
    /// Prints a character.
    fn print(&mut self, character: char) {
        self.print_char(character);
    }

    /// Carries out a control character.
    fn execute(&mut self, byte: u8) {
        let fill = self.fill();
        match byte {
            0x08 => {
                let grid = self.grid();
                grid.cursor.column = grid.cursor.column.saturating_sub(1);
                grid.cursor.pending_wrap = false;
            }
            0x09 => {
                let column = self.grid().cursor.column;
                let target = self.next_tab(column);
                let grid = self.grid();
                grid.cursor.column = target;
                grid.cursor.pending_wrap = false;
            }
            0x0a..=0x0c => {
                self.grid().line_feed(fill);
                self.grid().cursor.pending_wrap = false;
            }
            0x0d => {
                let grid = self.grid();
                grid.cursor.column = 0;
                grid.cursor.pending_wrap = false;
            }
            0x0e => self.charset = 1,
            0x0f => self.charset = 0,
            _ => {}
        }
    }

    /// Carries out an operating-system command: the window's title.
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if let [code, title, ..] = params
            && (*code == b"0" || *code == b"2")
        {
            self.title = Some(String::from_utf8_lossy(title).into_owned());
        }
    }

    /// Carries out a control sequence.
    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if ignore {
            return;
        }
        let private = intermediates.first() == Some(&b'?');
        let fill = self.fill();
        match (action, intermediates) {
            ('m', []) => self.graphics(params),
            ('h', _) => self.set_modes(params, private, true),
            ('l', _) => self.set_modes(params, private, false),
            ('A', []) => {
                let grid = self.grid();
                let top = if grid.cursor.row >= grid.top {
                    grid.top
                } else {
                    0
                };
                grid.cursor.row = grid.cursor.row.saturating_sub(first(params, 1)).max(top);
                grid.cursor.pending_wrap = false;
            }
            ('B', []) | ('e', []) => {
                let grid = self.grid();
                let bottom = if grid.cursor.row <= grid.bottom {
                    grid.bottom
                } else {
                    grid.rows - 1
                };
                grid.cursor.row = (grid.cursor.row + first(params, 1)).min(bottom);
                grid.cursor.pending_wrap = false;
            }
            ('C', []) | ('a', []) => {
                let grid = self.grid();
                grid.cursor.column = (grid.cursor.column + first(params, 1)).min(grid.columns - 1);
                grid.cursor.pending_wrap = false;
            }
            ('D', []) => {
                let grid = self.grid();
                grid.cursor.column = grid.cursor.column.saturating_sub(first(params, 1));
                grid.cursor.pending_wrap = false;
            }
            ('E', []) => {
                let row = self.grid().cursor.row + first(params, 1);
                self.move_to(row, 0);
            }
            ('F', []) => {
                let row = self.grid().cursor.row.saturating_sub(first(params, 1));
                self.move_to(row, 0);
            }
            ('G', []) | ('`', []) => {
                let row = self.grid().cursor.row;
                self.move_to(row, first(params, 1) - 1);
            }
            ('d', []) => {
                let column = self.grid().cursor.column;
                self.move_to(first(params, 1) - 1, column);
            }
            ('H', []) | ('f', []) => self.move_to(nth(params, 0, 1) - 1, nth(params, 1, 1) - 1),
            ('I', []) => {
                for _ in 0..first(params, 1) {
                    let column = self.grid().cursor.column;
                    let target = self.next_tab(column);
                    self.grid().cursor.column = target;
                }
            }
            ('Z', []) => {
                for _ in 0..first(params, 1) {
                    let column = self.grid().cursor.column;
                    let target = self.previous_tab(column);
                    self.grid().cursor.column = target;
                }
            }
            ('J', _) => self.erase_display(params.iter().next().map_or(0, |group| group[0])),
            ('K', _) => self.erase_line(params.iter().next().map_or(0, |group| group[0])),
            ('L', []) => self.grid().insert_lines(first(params, 1), fill),
            ('M', []) => self.grid().delete_lines(first(params, 1), fill),
            ('@', []) => self.grid().insert_cells(first(params, 1), fill),
            ('P', []) => self.grid().delete_cells(first(params, 1), fill),
            ('X', []) => {
                let grid = self.grid();
                let (row, column) = (grid.cursor.row, grid.cursor.column);
                grid.erase(row, column, column + first(params, 1), fill);
            }
            ('S', []) => self.grid().scroll_up(first(params, 1), fill),
            ('T', []) => self.grid().scroll_down(first(params, 1), fill),
            ('b', []) => {
                for _ in 0..first(params, 1) {
                    self.print_char(self.last);
                }
            }
            ('r', []) => {
                let grid = self.grid();
                let top = nth(params, 0, 1) - 1;
                let bottom = nth(params, 1, grid.rows).min(grid.rows) - 1;
                if top < bottom {
                    grid.top = top;
                    grid.bottom = bottom;
                }
                self.move_to(0, 0);
            }
            ('s', []) => {
                let grid = self.grid();
                grid.saved = grid.cursor;
            }
            ('u', []) => {
                let grid = self.grid();
                grid.cursor = Cursor {
                    pending_wrap: false,
                    ..grid.saved
                };
            }
            ('n', []) => match first(params, 0) {
                5 => self.reply("\x1b[0n"),
                6 => {
                    let at = self.grid().cursor;
                    self.reply(&format!("\x1b[{};{}R", at.row + 1, at.column + 1));
                }
                _ => {}
            },
            ('c', []) if nth(params, 0, 0) == 0 => self.reply("\x1b[?62;c"),
            ('c', [b'>']) => self.reply("\x1b[>0;0;0c"),
            ('g', []) => {
                let column = self.grid().cursor.column;
                match first(params, 0) {
                    0 => self.tabs[column] = false,
                    3 => self.tabs.fill(false),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Carries out an escape sequence.
    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if ignore {
            return;
        }
        let fill = self.fill();
        match (intermediates, byte) {
            ([], b'7') => {
                let grid = self.grid();
                grid.saved = grid.cursor;
            }
            ([], b'8') => {
                let grid = self.grid();
                grid.cursor = Cursor {
                    pending_wrap: false,
                    ..grid.saved
                };
            }
            ([], b'D') => self.grid().line_feed(fill),
            ([], b'E') => {
                self.grid().line_feed(fill);
                self.grid().cursor.column = 0;
            }
            ([], b'M') => self.grid().reverse_line_feed(fill),
            ([], b'H') => {
                let column = self.grid().cursor.column;
                self.tabs[column] = true;
            }
            ([], b'c') => self.reset(),
            ([b'('], set) => {
                self.charsets[0] = if set == b'0' {
                    Charset::Lines
                } else {
                    Charset::Plain
                }
            }
            ([b')'], set) => {
                self.charsets[1] = if set == b'0' {
                    Charset::Lines
                } else {
                    Charset::Plain
                }
            }
            _ => {}
        }
    }
}
