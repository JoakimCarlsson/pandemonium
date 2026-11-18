//! The terminal dock under the session: tabs of shells above the window of the one showing.

use crate::kit::sp;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use std::hash::{Hash, Hasher};

use pm_config::TerminalCursor;
use pm_terminal::{Cell, Color as Paint, Emulator, Flags, Link};
use rustle::{CursorIcon, Modifier};

use crate::Button;
use crate::dock::{BottomTab, TerminalTab};
use crate::kit::{
    FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Side, Styled, Tone, Type, label,
    label_with, measure,
};
use crate::parts::{glyph, line, spacer};
use crate::sample::Session;
use crate::state::{
    BOTTOM_PROBLEMS, BOTTOM_TERMINAL, MAXIMIZE_TERMINAL, NEW_TERMINAL, TERMINAL, TERMINAL_BODY,
    TERMINAL_CLOSE_BASE, TERMINAL_TAB_BASE, WorkspaceState,
};

/// How tall the dock is until it is dragged taller or shorter, in logical pixels.
pub const HEIGHT: f32 = 280.0;

/// How short and how tall the dock can be dragged, in logical pixels; a page leaves it less room than the second.
pub const HEIGHTS: (f32, f32) = (120.0, 2000.0);

/// How much of the page the dock always leaves above it, in logical pixels.
const ROOM_ABOVE: f32 = 160.0;

/// How much of the page the dock takes while it is maximized.
const MAXIMIZED: f32 = 0.7;

/// How far the window of the shell is from the sides of the dock, in logical pixels.
pub const PADDING_X: f32 = 16.0;

/// How far the window of the shell is from the top and bottom of the dock, in logical pixels.
pub const PADDING_Y: f32 = 12.0;

/// How tall a line is, as a share of the type.
const LEADING: f32 = 1.5;

/// How many characters are measured to find how wide one is.
const SAMPLE: usize = 64;

/// How a shell is drawn: how tall its type is, in logical pixels, and how its cursor looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// How tall the type is.
    pub font: f32,
    /// How the cursor looks.
    pub cursor: TerminalCursor,
}

impl Look {
    /// How the shells of `state` are drawn: in the terminal's own size or the code font's, with the cursor its settings say.
    pub fn of(state: &WorkspaceState) -> Self {
        Self {
            font: state
                .window
                .terminal
                .font_size
                .unwrap_or(state.editor.font_size) as f32,
            cursor: state.window.terminal.cursor,
        }
    }
}

/// How wide and how tall one cell of the shell's window is, in logical pixels, with type `font` tall.
pub fn cell_size(font: f32) -> (f32, f32) {
    let (width, height) = measure(&"M".repeat(SAMPLE), cell_type(font));
    (width / SAMPLE as f32, height)
}

/// The type the cells of a shell are set in: monospaced, `font` logical pixels tall, lines [`LEADING`] times that.
fn cell_type(font: f32) -> Type {
    Type::code(font).ratio(LEADING)
}

/// How many columns and lines fit in the window of a shell that is `width` by `height` logical pixels with its padding.
pub fn fitting(width: f32, height: f32, cell: (f32, f32)) -> (u16, u16) {
    let columns = ((width - 2.0 * PADDING_X) / cell.0).floor().max(1.0);
    let rows = ((height - 2.0 * PADDING_Y) / cell.1).floor().max(1.0);
    (columns.min(500.0) as u16, rows.min(200.0) as u16)
}

/// How tall the dock can be on a page `page` logical pixels tall.
pub fn tallest(page: f32) -> f32 {
    (page - ROOM_ABOVE).clamp(HEIGHTS.0, HEIGHTS.1)
}

/// How tall the dock is on a page `page` logical pixels tall.
pub fn height(state: &WorkspaceState, page: f32) -> f32 {
    let height = if state.terminal_maximized {
        (page * MAXIMIZED).max(state.terminal_height)
    } else {
        state.terminal_height
    };
    height.clamp(HEIGHTS.0, tallest(page))
}

/// How wide the list of shells at the right of the dock is, in logical pixels.
const SHELL_LIST_WIDTH: f32 = 160.0;

/// Emits the dock of `session`, on a page `width` wide and `page` tall.
#[track_caller]
pub fn terminal(state: &WorkspaceState, session: &Session, width: f32, page: f32) {
    let shown = state.bottom_tab();
    let (errors, warnings) = crate::problems_panel::tally(state);
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .height(height(state, page))
            .fill(Role::Background)
            .stroke_side(Side::Top, 1.0, Role::Border),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .h(10)
                    .stroke_side(Side::Bottom, 1.0, Role::Border)
                    .pad_x(2),
                Arrangement::SpacedBy(sp(1)),
                Alignment::Center,
                || {
                    pane_tab(
                        "Problems",
                        Some(errors + warnings).filter(|count| *count > 0),
                        shown == BottomTab::Problems,
                        BOTTOM_PROBLEMS,
                    );
                    if state.debug_docked() {
                        pane_tab(
                            "Debug Console",
                            None,
                            shown == BottomTab::Debug,
                            crate::debug::DEBUG_TAB,
                        );
                    }
                    pane_tab(
                        "Terminal",
                        None,
                        shown == BottomTab::Terminal,
                        BOTTOM_TERMINAL,
                    );
                    spacer();
                    if shown == BottomTab::Terminal {
                        Button::icon(IconName::Plus, NEW_TERMINAL).build();
                    }
                    Button::icon(
                        if state.terminal_maximized {
                            IconName::Minimize
                        } else {
                            IconName::Maximize
                        },
                        MAXIMIZE_TERMINAL,
                    )
                    .build();
                    Button::icon(IconName::Close, TERMINAL).build();
                },
            );
            match shown {
                BottomTab::Problems => crate::problems_panel::problems_tab(state),
                BottomTab::Debug => crate::debug_panel::debug_tab(state),
                BottomTab::Terminal => {
                    if let Some(tab) = session.terminals.get(session.terminal_active) {
                        let listed = session.terminals.len() > 1;
                        let room = if listed {
                            width - SHELL_LIST_WIDTH
                        } else {
                            width
                        };
                        row_arranged(
                            Modifier::new().fill_max_width().weight(1.0),
                            Arrangement::Start,
                            Alignment::Start,
                            || {
                                window(tab, room - 2.0 * PADDING_X, Look::of(state));
                                if listed {
                                    shell_list(session);
                                }
                            },
                        );
                    }
                }
            }
        },
    );
}

/// Emits a tab of the dock named `label`, filled while it is the one showing, with how many things it holds when it holds any.
#[track_caller]
fn pane_tab(label: &str, count: Option<usize>, shown: bool, id: FocusId) {
    let tone = if shown { Role::Text } else { Role::Muted };
    let looks = if shown {
        Looks::new().idle(Role::SurfaceSelected)
    } else {
        Looks::new().hover(Role::SurfaceHover)
    }
    .radius(Radius::Lg);
    row_arranged(
        Modifier::new()
            .h(8)
            .looks_of(looks, id)
            .cursor(CursorIcon::Pointer)
            .target(id)
            .pad_x(3),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            line(label, 13, 1.4, tone);
            if let Some(count) = count {
                row_arranged(
                    Modifier::new()
                        .fill_rounded(Role::Accent, Radius::Lg)
                        .pad_x(1.5),
                    Arrangement::Start,
                    Alignment::Center,
                    || line(count.to_string(), 11, 1.3, Role::OnAccent),
                );
            }
        },
    );
}

/// Emits the shells of `session` as a list at the right of the dock, the one showing marked, each with the button that ends it.
#[track_caller]
fn shell_list(session: &Session) {
    column_arranged(
        Modifier::new()
            .fill_max_height()
            .width(SHELL_LIST_WIDTH)
            .stroke_side(Side::Left, 1.0, Role::Border)
            .clip_to(Radius::None),
        Arrangement::Start,
        Alignment::Start,
        || {
            for (index, tab) in session.terminals.iter().enumerate() {
                rustle::key(index, || {
                    shell_row(index, tab, index == session.terminal_active)
                });
            }
        },
    );
}

/// Emits the row of the shell at `index`: its name, pressed to show it, and the button that ends it.
#[track_caller]
fn shell_row(index: usize, tab: &TerminalTab, shown: bool) {
    let tone = if shown { Role::Text } else { Role::Muted };
    let looks = if shown {
        Looks::new().idle(Role::SurfaceSelected)
    } else {
        Looks::new().hover(Role::SurfaceHover)
    };
    let select = FocusId(TERMINAL_TAB_BASE + index as u64);
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .h(7)
            .looks(looks)
            .pad_side(Side::Right, 1),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new()
                    .weight(1.0)
                    .h(7)
                    .clip_to(Radius::None)
                    .cursor(CursorIcon::Pointer)
                    .target(select)
                    .pad_side(Side::Left, 3),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    glyph(IconName::Terminal, 3.5, tone);
                    line(tab.name(), 13, 1.4, tone);
                },
            );
            Button::icon(IconName::Close, FocusId(TERMINAL_CLOSE_BASE + index as u64)).build();
        },
    );
}

/// Emits the window of the shell `tab`, `width` logical pixels wide inside its padding and drawn as `look` says, which takes the keyboard when pressed.
#[track_caller]
fn window(tab: &TerminalTab, width: f32, look: Look) {
    let emulator = tab.emulator.clone();
    let back = tab.back;
    let composing = tab.composing.clone();
    let link = tab.link.clone();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (
        tab.id,
        emulator.read().version(),
        back,
        &composing,
        &link,
        look.font.to_bits(),
        look.cursor as u8,
    )
        .hash(&mut hasher);
    let key = hasher.finish();
    column_arranged(
        Modifier::new()
            .fill_max_height()
            .weight(1.0)
            .fill(Role::Background)
            .clip_to(Radius::None)
            .cursor(CursorIcon::Text)
            .target(TERMINAL_BODY)
            .padding_xy(PADDING_X, PADDING_Y),
        Arrangement::Start,
        Alignment::Start,
        move || {
            rustle::key(key, || {
                rustle::skippable((key, width.to_bits()), || {
                    lines(&emulator.read(), back, &composing, link.as_ref(), look);
                });
            });
        },
    );
}

/// What the cells of a run share, which is what makes them one run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Run {
    /// The foreground the cell asks for.
    fg: Paint,
    /// The background the cell asks for.
    bg: Paint,
    /// How the character is drawn.
    flags: Flags,
    /// Whether the reader marked the cell.
    selected: bool,
    /// Whether the cursor is on the cell.
    cursor: bool,
    /// Whether the cell is part of the link a press opens.
    linked: bool,
}

/// Emits the lines of the window of `emulator` scrolled `back` lines up, with `composing` shown at the cursor and `link` underlined, drawn as `look` says.
#[track_caller]
fn lines(emulator: &Emulator, back: usize, composing: &str, link: Option<&Link>, look: Look) {
    let first = emulator.first_line(back);
    let cursor = emulator.cursor(back);
    let selection = emulator.selection().map(|marked| marked.ordered());
    column_arranged(
        Modifier::new(),
        Arrangement::Start,
        Alignment::Start,
        || {
            for row in 0..emulator.rows() {
                let at_cursor = cursor
                    .filter(|(line, _)| *line == row)
                    .map(|(_, column)| column);
                let marked = selection.and_then(|range| marked_columns(range, first + row));
                let linked = link.and_then(|link| linked_columns(link, first + row));
                rustle::key(row, || {
                    line_of(
                        emulator.line(row, back),
                        at_cursor,
                        Marks { marked, linked },
                        composing,
                        look,
                    )
                });
            }
        },
    );
}

/// The columns of line `line` that the selection from `start` to `end` covers, when it covers any.
fn marked_columns(
    (start, end): ((usize, usize), (usize, usize)),
    line: usize,
) -> Option<(usize, usize)> {
    if line < start.0 || line > end.0 {
        return None;
    }
    let from = if line == start.0 { start.1 } else { 0 };
    let to = if line == end.0 { end.1 } else { usize::MAX };
    Some((from, to))
}

/// The columns of line `line` that `link` covers, when it covers any.
fn linked_columns(link: &Link, line: usize) -> Option<(usize, usize)> {
    marked_columns((link.start, link.end), line)
}

/// The columns of one line that are drawn apart from the rest, each from one column to another inclusive.
#[derive(Clone, Copy, Debug, Default)]
struct Marks {
    /// The columns the reader marked.
    marked: Option<(usize, usize)>,
    /// The columns of the link a press opens.
    linked: Option<(usize, usize)>,
}

/// Emits one line of the window: its cells grouped into runs of the same look, with `composing` drawn just before the cursor, drawn as `look` says.
#[track_caller]
fn line_of(
    cells: &[Cell],
    cursor: Option<usize>,
    Marks { marked, linked }: Marks,
    composing: &str,
    look: Look,
) {
    let used = cells
        .iter()
        .rposition(|cell| !cell.is_blank())
        .map_or(0, |last| last + 1);
    let end = marked
        .map_or(used, |(_, to)| {
            used.max(to.saturating_add(1).min(cells.len()))
        })
        .max(cursor.map_or(0, |column| column + 1))
        .min(cells.len());
    row_arranged(
        Modifier::new().height(look.font * LEADING),
        Arrangement::Start,
        Alignment::Start,
        || {
            let mut run: Option<(Run, String)> = None;
            for (column, cell) in cells[..end].iter().enumerate() {
                let shared = Run {
                    fg: cell.fg,
                    bg: cell.bg,
                    flags: cell.flags,
                    selected: marked.is_some_and(|(from, to)| column >= from && column <= to),
                    cursor: cursor == Some(column),
                    linked: linked.is_some_and(|(from, to)| column >= from && column <= to),
                };
                let character = if cell.flags.has(Flags::HIDDEN) {
                    ' '
                } else {
                    cell.ch
                };
                match &mut run {
                    Some((held, content)) if *held == shared && !shared.cursor => {
                        content.push(character)
                    }
                    _ => {
                        if let Some((held, content)) = run.take() {
                            run_node(held, content, look);
                        }
                        if shared.cursor && !composing.is_empty() {
                            composing_node(composing, look.font);
                        }
                        run = Some((shared, character.to_string()));
                    }
                }
            }
            if let Some((held, content)) = run {
                run_node(held, content, look);
            }
        },
    );
}

/// Emits `composing`, text an input method has not committed yet, underlined in the default colours, `font` logical pixels tall.
#[track_caller]
fn composing_node(composing: &str, font: f32) {
    label(composing, cell_type(font).tone(Role::Text).underline());
}

/// How a paint is shown: a colour, or the default one of `role`.
fn shown(paint: Paint, role: Role) -> Tone {
    match paint {
        Paint::Default => role.into(),
        Paint::Indexed(index) => {
            let (red, green, blue) = pm_terminal::indexed(index);
            Tone::rgb(red, green, blue)
        }
        Paint::Rgb(red, green, blue) => Tone::rgb(red, green, blue),
    }
}

/// Emits `content` as the cells of `look` share, with type and cursor as `drawn` says.
#[track_caller]
fn run_node(look: Run, content: String, drawn: Look) {
    let inverse = look.flags.has(Flags::INVERSE);
    let (mut ink, mut ground) = if inverse {
        (
            shown(look.bg, Role::Background),
            Some(shown(look.fg, Role::Text)),
        )
    } else {
        (
            shown(look.fg, Role::Text),
            (look.bg != Paint::Default).then(|| shown(look.bg, Role::Background)),
        )
    };
    if look.flags.has(Flags::DIM) {
        ink = ink.alpha(0.6);
    }
    if look.selected {
        ground = Some(Role::SurfaceActive.into());
    }
    let mut kind = cell_type(drawn.font);
    if look.flags.has(Flags::BOLD) {
        kind = kind.bold();
    }
    if look.flags.has(Flags::ITALIC) {
        kind = kind.italic();
    }
    if look.flags.has(Flags::UNDERLINE) || look.linked {
        kind = kind.underline();
    }
    if look.flags.has(Flags::STRIKE) {
        kind = kind.strike();
    }
    let mut modifier = Modifier::new().fill_max_height();
    if let Some(ground) = ground {
        modifier = modifier.fill(ground);
    }
    if look.cursor {
        let (modifier, ink) = cursor_cell(modifier, ink, drawn.cursor);
        label_with(modifier, content, kind.tone(ink));
        return;
    }
    label_with(modifier, content, kind.tone(ink));
}

/// `modifier`, of the cell under the cursor, with the cursor drawn as `shape` says: subtle, and in the text colour while the window holds the keyboard; and the ink its character takes instead of `ink`.
fn cursor_cell(modifier: Modifier, ink: Tone, shape: TerminalCursor) -> (Modifier, Tone) {
    let modifier = modifier.focus_of(TERMINAL_BODY);
    match shape {
        TerminalCursor::Block => (
            modifier.looks(Looks::new().idle(Role::Subtle).focus(Role::Text)),
            Role::Background.into(),
        ),
        TerminalCursor::Bar => (
            cursor_edge(modifier, Side::Left).padding_each(2.0, 0.0, 0.0, 0.0),
            ink,
        ),
        TerminalCursor::Underline => (
            cursor_edge(modifier, Side::Bottom).padding_each(0.0, 0.0, 0.0, 2.0),
            ink,
        ),
    }
}

/// `modifier` with a line two pixels thick along its `side`, subtle, and in the text colour while the window holds the keyboard.
fn cursor_edge(modifier: Modifier, side: Side) -> Modifier {
    let (idle, focused) = (
        crate::kit::color(Role::Subtle),
        crate::kit::color(Role::Text),
    );
    modifier.styled_by(move |canvas, interaction| {
        let color = if interaction.focused { focused } else { idle };
        let (width, height) = (canvas.width(), canvas.height());
        match side {
            Side::Bottom => canvas.rect(0.0, height - 2.0, width, 2.0, color),
            _ => canvas.rect(0.0, 0.0, 2.0, height, color),
        }
    })
}

/// The line of the window and the column that the point `x`, `y` logical pixels from the top left of the window of a shell is over, kept inside a window of `columns` by `rows`.
pub fn cell_at(
    (x, y): (f32, f32),
    cell: (f32, f32),
    (columns, rows): (usize, usize),
) -> (usize, usize) {
    let column = ((x - PADDING_X) / cell.0).floor().max(0.0) as usize;
    let row = ((y - PADDING_Y) / cell.1).floor().max(0.0) as usize;
    (
        row.min(rows.saturating_sub(1)),
        column.min(columns.saturating_sub(1)),
    )
}
