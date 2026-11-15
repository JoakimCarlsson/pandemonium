//! A terminal window: the cell grid of a shell drawn in the monospace face
//! with its colours, a cursor and the marked text, which sends the keys typed
//! into it while it holds focus, marks text under the pointer, scrolls back
//! with the wheel and offers a menu on the secondary button.

use std::rc::Rc;
use std::time::{Duration, Instant};

use pm_terminal::{Cell, Color as CellColour, Flags, indexed};
use rustle::prelude::*;
use rustle::{KeyEvent, Offset, PointerButton, layout_text};

use crate::option_list::OptionList;
use crate::size::Size;
use crate::terminal_keys::{TerminalCommand, TerminalShortcut, key_bytes, terminal_shortcut};
use rustle::{ColorScheme, color_scheme};

/// How a terminal view sits on the page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerminalViewVariant {
    /// On the page's background, as in the dock.
    #[default]
    Plain,
    /// On a raised surface with a border, as an inset beside other content.
    Inset,
}

/// A stretch of the window the pointer marked, in lines and columns of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSelection {
    /// The cell the press went down on.
    pub anchor: (usize, usize),
    /// The cell the pointer is over now.
    pub head: (usize, usize),
    /// How many presses in a row began it: one marks cells, two words, three lines.
    pub clicks: u32,
}

/// The entries of the menu the secondary button opens, in order, with the command each runs.
const MENU: [(&str, TerminalCommand); 4] = [
    ("Copy", TerminalCommand::Copy),
    ("Paste", TerminalCommand::Paste),
    ("Select all", TerminalCommand::SelectAll),
    ("Clear", TerminalCommand::Clear),
];

/// How soon a press must follow the last on the same cell to count as the next in a row.
const MULTI_CLICK: Duration = Duration::from_millis(450);

/// The callbacks a terminal view hands what the reader did to.
#[derive(Clone, Default)]
struct Callbacks {
    /// What receives the bytes the keys typed into it send.
    on_input: Option<Rc<dyn Fn(Vec<u8>)>>,
    /// What learns how many columns and rows fit, each time it is laid out.
    on_resize: Option<Rc<dyn Fn(u16, u16)>>,
    /// What learns of each change to the stretch the pointer marks.
    on_select: Option<Rc<dyn Fn(TerminalSelection)>>,
    /// What scrolls the view, by lines back when positive and forward when negative.
    on_scroll: Option<Rc<dyn Fn(i32)>>,
    /// What runs the commands of the menu and the shortcuts.
    on_command: Option<Rc<dyn Fn(TerminalCommand)>>,
}

/// A shell's screen with a variant and a size.
#[derive(Clone)]
pub struct TerminalView {
    /// The cells of each line, top first.
    lines: Rc<Vec<Vec<Cell>>>,
    /// The line and column of the cursor, when it is drawn.
    cursor: Option<(usize, usize)>,
    /// The first and last marked cell, each a line of the window and a column, in reading order.
    selection: Option<((usize, usize), (usize, usize))>,
    /// Whether the arrow keys send the application form.
    application_cursor: bool,
    /// How it sits on the page.
    variant: TerminalViewVariant,
    /// Its size, which sets the type size.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What it tells of what the reader does.
    callbacks: Callbacks,
}

impl TerminalView {
    /// A plain view of `lines` with no cursor.
    pub fn new(lines: Vec<Vec<Cell>>) -> Self {
        Self {
            lines: Rc::new(lines),
            cursor: None,
            selection: None,
            application_cursor: false,
            variant: TerminalViewVariant::Plain,
            size: Size::Sm,
            modifier: Modifier::new(),
            callbacks: Callbacks::default(),
        }
    }

    /// The view with its cursor at `cursor`, a line and a column, or none.
    pub fn cursor(mut self, cursor: Option<(usize, usize)>) -> Self {
        self.cursor = cursor;
        self
    }

    /// The view marking the cells from the first to the last of `selection`, each a line of the window and a column.
    pub fn selection(mut self, selection: Option<((usize, usize), (usize, usize))>) -> Self {
        self.selection = selection;
        self
    }

    /// The view sending the application form of the arrow keys when `on`.
    pub fn application_cursor(mut self, on: bool) -> Self {
        self.application_cursor = on;
        self
    }

    /// The view in `variant`.
    pub fn variant(mut self, variant: TerminalViewVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The view at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The view laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The view handing the bytes of each key typed into it to `on_input`.
    pub fn on_input(mut self, on_input: impl Fn(Vec<u8>) + 'static) -> Self {
        self.callbacks.on_input = Some(Rc::new(on_input));
        self
    }

    /// The view telling `on_resize` how many columns and rows fit in it.
    pub fn on_resize(mut self, on_resize: impl Fn(u16, u16) + 'static) -> Self {
        self.callbacks.on_resize = Some(Rc::new(on_resize));
        self
    }

    /// The view telling `on_select` of each change to the stretch the pointer marks.
    pub fn on_select(mut self, on_select: impl Fn(TerminalSelection) + 'static) -> Self {
        self.callbacks.on_select = Some(Rc::new(on_select));
        self
    }

    /// The view asking `on_scroll` to scroll it, by lines back when positive and forward when negative.
    pub fn on_scroll(mut self, on_scroll: impl Fn(i32) + 'static) -> Self {
        self.callbacks.on_scroll = Some(Rc::new(on_scroll));
        self
    }

    /// The view handing the commands of its menu and shortcuts to `on_command`.
    pub fn on_command(mut self, on_command: impl Fn(TerminalCommand) + 'static) -> Self {
        self.callbacks.on_command = Some(Rc::new(on_command));
        self
    }

    /// The text style the cells are set in.
    pub fn style(&self) -> TextStyle {
        let code = crate::code_font();
        let font_size = match self.size {
            Size::Xs => 11.0,
            Size::Sm => 12.0,
            Size::Md => 13.0,
            Size::Lg => 15.0,
        } * code.terminal_size
            / 12.0;
        TextStyle {
            color: color_scheme().on_surface,
            font_size,
            line_height: code.line_for(font_size, 1.35),
            font_family: code.family,
            max_lines: Some(1),
            ligatures: false,
            ..TextStyle::default()
        }
    }

    /// Emits the view.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let style = self.style();
        let cell = cell_size(&style);
        let focus = remember_focus();
        let focused = focus.is_focused();
        let menu = remember(|| None::<Offset>);
        let pointer = Pointer {
            anchor: remember(|| None::<(usize, usize)>),
            clicks: remember(|| 1_u32),
            last: remember(|| None::<(Instant, (usize, usize))>),
            remainder: remember(|| 0.0_f32),
            menu: menu.clone(),
            focus: focus.clone(),
            cell,
            rows: self.lines.len(),
            callbacks: self.callbacks.clone(),
        };
        let application_cursor = self.application_cursor;
        let callbacks = self.callbacks.clone();
        let rows = self.lines.len();
        let on_resize = self.callbacks.on_resize.clone();
        let modifier = match self.variant {
            TerminalViewVariant::Plain => self.modifier.background(scheme.background),
            TerminalViewVariant::Inset => self
                .modifier
                .background(scheme.surface_container)
                .border(1.0, scheme.outline_variant, 6.0),
        };
        let modifier = modifier
            .clip()
            .on_size_changed(move |size| {
                if let Some(on_resize) = &on_resize {
                    on_resize(
                        (size.width / cell.0).floor() as u16,
                        (size.height / cell.1).floor() as u16,
                    );
                }
            })
            .pointer_input(move |input| pointer.handle(input))
            .on_context_menu({
                let menu = menu.clone();
                move |event| menu.set(Some(event.position))
            })
            .focusable(&focus, move |event| {
                key_pressed(&callbacks, event, application_cursor, rows)
            })
            .cursor(CursorIcon::Text);
        let screen = Screen {
            lines: self.lines,
            cursor: self.cursor,
            selection: self.selection,
            focused,
            cell,
            style,
            scheme,
            shape: crate::code_look().terminal_cursor,
        };
        let on_command = self.callbacks.on_command.clone();
        stack_with(modifier, move || {
            canvas(Modifier::new().fill_max_size(), move |canvas| {
                screen.draw(canvas)
            });
            if let Some(at) = menu.get() {
                stack_with(Modifier::new().offset(at.x, at.y), || {
                    let close = menu.clone();
                    let picked = menu.clone();
                    popup(
                        move || close.set(None),
                        move || {
                            OptionList::new(
                                MENU.iter().map(|(label, _)| (*label).to_owned()).collect(),
                                None,
                            )
                            .size(Size::Xs)
                            .modifier(Modifier::new().width(160.0))
                            .on_pick(move |place| {
                                picked.set(None);
                                if let Some(on_command) = &on_command {
                                    on_command(MENU[place].1);
                                }
                            })
                            .show();
                        },
                    );
                });
            }
        });
    }
}

/// Hands the key `event` to the terminal's shortcuts first, then to the shell; returns whether either took it.
fn key_pressed(
    callbacks: &Callbacks,
    event: &KeyEvent,
    application_cursor: bool,
    rows: usize,
) -> bool {
    match terminal_shortcut(event) {
        Some(TerminalShortcut::Command(command)) => {
            if let Some(on_command) = &callbacks.on_command {
                on_command(command);
            }
            return true;
        }
        Some(TerminalShortcut::Page(pages)) => {
            if let Some(on_scroll) = &callbacks.on_scroll {
                on_scroll(pages * rows.max(1) as i32);
            }
            return true;
        }
        None => {}
    }
    match (&callbacks.on_input, key_bytes(event, application_cursor)) {
        (Some(on_input), Some(bytes)) => {
            on_input(bytes);
            true
        }
        _ => false,
    }
}

/// What the pointer handler of a terminal view keeps between events.
struct Pointer {
    /// The cell the primary button went down on, while it is held.
    anchor: State<Option<(usize, usize)>>,
    /// How many presses in a row the one held began with.
    clicks: State<u32>,
    /// When and on which cell the last press went down.
    last: State<Option<(Instant, (usize, usize))>>,
    /// The part of a line the wheel turned that has not scrolled yet.
    remainder: State<f32>,
    /// Where the menu is open, if it is; a press then belongs to the menu, not the screen beneath it.
    menu: State<Option<Offset>>,
    /// The view's focus, taken on a press.
    focus: Focus,
    /// The width and height of one cell.
    cell: (f32, f32),
    /// How many lines the window has.
    rows: usize,
    /// What it tells of what the reader does.
    callbacks: Callbacks,
}

impl Pointer {
    /// Handles one pointer event, returning whether a wheel turn was used.
    fn handle(&self, input: &PointerInput) -> bool {
        match input.phase {
            PointerInputPhase::Scroll => self.scroll(input.scroll_delta.y),
            PointerInputPhase::Down
                if input.button == PointerButton::Primary && self.menu.get().is_none() =>
            {
                self.press(self.cell_at(input.position));
                false
            }
            PointerInputPhase::Move => {
                if let Some(anchor) = self.anchor.get() {
                    self.select(anchor, self.cell_at(input.position));
                }
                false
            }
            PointerInputPhase::Up | PointerInputPhase::Cancel => {
                self.anchor.set(None);
                false
            }
            _ => false,
        }
    }

    /// Starts marking at `at`, counting the press as the next in a row when it follows the last soon on the same cell.
    fn press(&self, at: (usize, usize)) {
        self.focus.request_focus();
        let now = Instant::now();
        let again = self
            .last
            .get()
            .is_some_and(|(when, cell)| cell == at && now.duration_since(when) < MULTI_CLICK);
        self.clicks
            .set(if again { self.clicks.get() % 3 + 1 } else { 1 });
        self.last.set(Some((now, at)));
        self.anchor.set(Some(at));
        self.select(at, at);
    }

    /// Tells of the stretch from `anchor` to `head`.
    fn select(&self, anchor: (usize, usize), head: (usize, usize)) {
        if let Some(on_select) = &self.callbacks.on_select {
            on_select(TerminalSelection {
                anchor,
                head,
                clicks: self.clicks.get(),
            });
        }
    }

    /// Scrolls by the whole lines `delta` pixels of wheel make with what was left over, returning whether it scrolled.
    fn scroll(&self, delta: f32) -> bool {
        let Some(on_scroll) = &self.callbacks.on_scroll else {
            return false;
        };
        let lines = self.remainder.get() + delta / self.cell.1;
        let whole = lines.trunc();
        self.remainder.set(lines - whole);
        if whole != 0.0 {
            on_scroll(whole as i32);
        }
        true
    }

    /// The line and column of the cell under `position`, kept inside the window.
    fn cell_at(&self, position: Offset) -> (usize, usize) {
        let row = (position.y.max(0.0) / self.cell.1) as usize;
        let column = (position.x.max(0.0) / self.cell.0) as usize;
        (row.min(self.rows.saturating_sub(1)), column)
    }
}

/// What the canvas of a terminal view draws, taken out of the builder.
struct Screen {
    /// The cells of each line, top first.
    lines: Rc<Vec<Vec<Cell>>>,
    /// The line and column of the cursor, when it is drawn.
    cursor: Option<(usize, usize)>,
    /// The first and last marked cell, in reading order.
    selection: Option<((usize, usize), (usize, usize))>,
    /// Whether the view holds focus, which fills the cursor.
    focused: bool,
    /// The width and height of one cell.
    cell: (f32, f32),
    /// The text style the cells are set in.
    style: TextStyle,
    /// The colours of the interface.
    scheme: ColorScheme,
    /// How the cursor is drawn while the view holds focus.
    shape: pm_editor::CursorShape,
}

impl Screen {
    /// Paints the backgrounds, the text and the cursor onto `canvas`.
    fn draw(&self, canvas: &mut Canvas) {
        let (width, height) = self.cell;
        for (row, line) in self.lines.iter().enumerate() {
            let y = row as f32 * height;
            for run in runs(line) {
                let looks = self.looks(&line[run.start]);
                let x = run.start as f32 * width;
                let span = (run.end - run.start) as f32 * width;
                if let Some(background) = looks.background {
                    canvas.rect(x, y, span, height, background);
                }
                let content: String = line[run.clone()].iter().map(|cell| cell.ch).collect();
                if looks.hidden || content.trim().is_empty() {
                    continue;
                }
                let style = TextStyle {
                    color: looks.foreground,
                    font_weight: looks.weight,
                    italic: looks.italic,
                    underline: looks.underline,
                    strikethrough: looks.strike,
                    ..self.style
                };
                canvas.draw_text(x, y, &content, &style, None);
            }
        }
        self.draw_selection(canvas);
        if let Some((row, column)) = self.cursor {
            self.draw_cursor(canvas, row, column);
        }
    }

    /// Tints the marked cells, line by line.
    fn draw_selection(&self, canvas: &mut Canvas) {
        let Some(((first_row, first_column), (last_row, last_column))) = self.selection else {
            return;
        };
        let (width, height) = self.cell;
        let tint = Color {
            a: 90,
            ..self.scheme.primary
        };
        for row in first_row..=last_row.min(self.lines.len().saturating_sub(1)) {
            let columns = self.lines[row].len();
            let from = if row == first_row { first_column } else { 0 };
            let to = if row == last_row {
                (last_column + 1).min(columns)
            } else {
                columns
            };
            if to > from {
                canvas.rect(
                    from as f32 * width,
                    row as f32 * height,
                    (to - from) as f32 * width,
                    height,
                    tint,
                );
            }
        }
    }

    /// Paints the cursor over the cell at `row` and `column`: a filled block
    /// with the character in the background colour while focused, an outline otherwise.
    fn draw_cursor(&self, canvas: &mut Canvas, row: usize, column: usize) {
        let (width, height) = self.cell;
        let x = column as f32 * width;
        let y = row as f32 * height;
        if !self.focused {
            canvas.stroke_rect(x, y, width, height, 0.0, 1.0, self.scheme.on_surface);
            return;
        }
        match self.shape {
            pm_editor::CursorShape::Bar => {
                canvas.rect(x, y, 2.0, height, self.scheme.on_surface);
                return;
            }
            pm_editor::CursorShape::Underline => {
                canvas.rect(x, y + height - 2.0, width, 2.0, self.scheme.on_surface);
                return;
            }
            pm_editor::CursorShape::Block => {}
        }
        canvas.rect(x, y, width, height, self.scheme.on_surface);
        let under = self
            .lines
            .get(row)
            .and_then(|line| line.get(column))
            .map_or(' ', |cell| cell.ch);
        if under != ' ' {
            let style = TextStyle {
                color: self.scheme.background,
                ..self.style
            };
            canvas.draw_text(x, y, &under.to_string(), &style, None);
        }
    }

    /// How `cell` is painted, with inverse, dim and bold applied.
    fn looks(&self, cell: &Cell) -> Looks {
        let foreground = resolve(cell.fg).unwrap_or(self.scheme.on_surface);
        let background = resolve(cell.bg);
        let (foreground, background) = if cell.flags.has(Flags::INVERSE) {
            (
                background.unwrap_or(self.scheme.background),
                Some(foreground),
            )
        } else {
            (foreground, background)
        };
        Looks {
            foreground: if cell.flags.has(Flags::DIM) {
                Color {
                    a: (f32::from(foreground.a) * 0.6) as u8,
                    ..foreground
                }
            } else {
                foreground
            },
            background,
            weight: if cell.flags.has(Flags::BOLD) {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            },
            italic: cell.flags.has(Flags::ITALIC),
            underline: cell.flags.has(Flags::UNDERLINE),
            strike: cell.flags.has(Flags::STRIKE),
            hidden: cell.flags.has(Flags::HIDDEN),
        }
    }
}

/// How a run of cells is painted.
struct Looks {
    /// The colour of the characters.
    foreground: Color,
    /// The colour behind them, when it is not the view's own.
    background: Option<Color>,
    /// The weight of the characters.
    weight: FontWeight,
    /// Whether the characters lean.
    italic: bool,
    /// Whether a line runs under them.
    underline: bool,
    /// Whether a line runs through them.
    strike: bool,
    /// Whether they are left undrawn.
    hidden: bool,
}

/// The colour a cell asks for, or none for the terminal's own.
fn resolve(colour: CellColour) -> Option<Color> {
    match colour {
        CellColour::Default => None,
        CellColour::Indexed(index) => {
            let (red, green, blue) = indexed(index);
            Some(Color::rgb(red, green, blue))
        }
        CellColour::Rgb(red, green, blue) => Some(Color::rgb(red, green, blue)),
    }
}

/// The ranges of `line` whose cells share colours and flags, left to right.
fn runs(line: &[Cell]) -> Vec<std::ops::Range<usize>> {
    let mut runs = Vec::new();
    let mut start = 0;
    for index in 1..=line.len() {
        let ends = index == line.len() || {
            let (left, right) = (&line[index - 1], &line[index]);
            (left.fg, left.bg, left.flags) != (right.fg, right.bg, right.flags)
        };
        if ends {
            runs.push(start..index);
            start = index;
        }
    }
    runs
}

/// The width and height of one cell in `style`, measured on a run of characters.
fn cell_size(style: &TextStyle) -> (f32, f32) {
    let sample = "0".repeat(64);
    let width = layout_text(&sample, style, None).size().width / 64.0;
    (width.max(1.0), style.line_height.max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cells split into runs where their colours or flags change.
    #[test]
    fn runs_split_on_style_changes() {
        let plain = Cell::default();
        let red = Cell {
            fg: CellColour::Indexed(1),
            ..Cell::default()
        };
        assert_eq!(runs(&[plain, plain, red, plain]), vec![0..2, 2..3, 3..4]);
        assert!(runs(&[]).is_empty());
    }
}
