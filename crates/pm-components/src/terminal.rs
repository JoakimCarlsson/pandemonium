//! The terminal dock under the session: tabs of shells above the window of the one showing.

use std::hash::{Hash, Hasher};

use pm_config::TerminalCursor;
use pm_icon::IconName;
use pm_style::typography::{FontFamily, LineHeight, TypographyStyle};
use pm_style::{
    Color, Radius, Role, Sides, bg, border_b, border_color, border_t, border_width, code_px,
    cursor_pointer, cursor_text, flex, flex_1, flex_col, flex_none, font_bold, font_mono, gap, h,
    h_px, italic, items_center, leading_ratio, line_through, min_h, overflow_hidden, pr, px,
    rounded, text_color, underline, w_full,
};
use pm_terminal::{Cell, Color as Paint, Emulator, Flags, Link};
use pm_text::Shaper;
use pm_ui::{Node, node, retained, text};

use crate::Button;
use crate::dock::TerminalTab;
use crate::parts::{glyph, line, spacer};
use crate::sample::Session;
use crate::state::{
    MAXIMIZE_TERMINAL, NEW_TERMINAL, TERMINAL, TERMINAL_BODY, TERMINAL_CLOSE_BASE,
    TERMINAL_TAB_BASE, WorkspaceState,
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
pub fn cell_size(shaper: &mut Shaper, font: f32) -> (f32, f32) {
    let style = TypographyStyle {
        family: FontFamily::Mono,
        size: font,
        line_height: LineHeight::Ratio(LEADING),
        ..TypographyStyle::BASE
    };
    let extent = shaper.measure(&"M".repeat(SAMPLE), &style);
    (extent.width / SAMPLE as f32, extent.height)
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

/// The dock of `session`, on a page `width` wide and `page` tall.
pub fn terminal(
    state: &WorkspaceState,
    session: &Session,
    width: f32,
    page: f32,
    shaper: &mut Shaper,
) -> Node {
    let mut strip = node().styles([
        flex(),
        items_center(),
        gap(1),
        h(10),
        px(2),
        flex_none(),
        border_b(),
        border_color(Role::Border),
    ]);
    let debugging = state.debug_docked();
    if debugging {
        strip = strip.child(debug_tab_button(state.debug.shown));
    }
    for (index, tab) in session.terminals.iter().enumerate() {
        let shown = index == session.terminal_active && !(debugging && state.debug.shown);
        strip = strip.child(tab_button(index, tab, shown));
    }
    strip = strip
        .child(Button::icon(IconName::Plus, NEW_TERMINAL).build())
        .child(spacer())
        .child(
            Button::icon(
                if state.terminal_maximized {
                    IconName::Minimize
                } else {
                    IconName::Maximize
                },
                MAXIMIZE_TERMINAL,
            )
            .build(),
        )
        .child(Button::icon(IconName::Close, TERMINAL).build());
    let mut dock = node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            w_full(),
            h_px(height(state, page)),
            bg(Role::Background),
            border_t(),
            border_color(Role::Border),
        ])
        .child(strip);
    if debugging && state.debug.shown {
        dock = dock.child(crate::debug_panel::debug_tab(state));
    } else if let Some(tab) = session.terminals.get(session.terminal_active) {
        dock = dock.child(window(
            tab,
            width - 2.0 * PADDING_X,
            Look::of(state),
            shaper,
        ));
    }
    dock
}

/// The tab of the program being debugged, filled while it is the one shown.
fn debug_tab_button(shown: bool) -> Node {
    let tone = if shown { Role::Text } else { Role::Muted };
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(8),
        px(3),
        rounded(Radius::Lg),
        text_color(tone),
        cursor_pointer(),
    ];
    if shown {
        styles.push(bg(Role::SurfaceSelected));
    } else {
        styles.push(bg(Role::SurfaceHover).hover());
    }
    node()
        .styles(styles)
        .child(glyph(IconName::Bug, 3.5, tone))
        .child(line("Debug", 13, 1.4, tone))
        .focusable(crate::debug::DEBUG_TAB)
}

/// The tab of the shell at `index`: its name, pressed to show it, and the button that ends it.
fn tab_button(index: usize, tab: &TerminalTab, shown: bool) -> Node {
    let tone = if shown { Role::Text } else { Role::Muted };
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(8),
        pr(1),
        rounded(Radius::Lg),
        text_color(tone),
    ];
    if shown {
        styles.push(bg(Role::SurfaceSelected));
    } else {
        styles.push(bg(Role::SurfaceHover).hover());
    }
    let name = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(8),
            pm_style::pl(3),
            cursor_pointer(),
        ])
        .child(glyph(IconName::Terminal, 3.5, tone))
        .child(line(tab.name(), 13, 1.4, tone))
        .focusable(pm_input::FocusId(TERMINAL_TAB_BASE + index as u64));
    node().styles(styles).child(name).child(
        Button::icon(
            IconName::Close,
            pm_input::FocusId(TERMINAL_CLOSE_BASE + index as u64),
        )
        .build(),
    )
}

/// The window of the shell `tab`, `width` logical pixels wide inside its padding and drawn as `look` says, which takes the keyboard when pressed.
fn window(tab: &TerminalTab, width: f32, look: Look, shaper: &mut Shaper) -> Node {
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
    let grid = retained(hasher.finish(), width, shaper, move |_| {
        lines(&emulator.read(), back, &composing, link.as_ref(), look)
    });
    node()
        .styles([
            flex(),
            flex_col(),
            flex_1(),
            min_h(0),
            overflow_hidden(),
            pm_style::padding(Sides::Horizontal, PADDING_X),
            pm_style::padding(Sides::Vertical, PADDING_Y),
            bg(Role::Background),
            cursor_text(),
        ])
        .child(grid)
        .focusable(TERMINAL_BODY)
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

/// The lines of the window of `emulator` scrolled `back` lines up, with `composing` shown at the cursor and `link` underlined, drawn as `look` says.
fn lines(
    emulator: &Emulator,
    back: usize,
    composing: &str,
    link: Option<&Link>,
    look: Look,
) -> Node {
    let first = emulator.first_line(back);
    let cursor = emulator.cursor(back);
    let selection = emulator.selection().map(|marked| marked.ordered());
    node()
        .styles([flex(), flex_col(), flex_none()])
        .children((0..emulator.rows()).map(|row| {
            let at_cursor = cursor
                .filter(|(line, _)| *line == row)
                .map(|(_, column)| column);
            let marked = selection.and_then(|range| marked_columns(range, first + row));
            let linked = link.and_then(|link| linked_columns(link, first + row));
            line_of(
                emulator.line(row, back),
                at_cursor,
                Marks { marked, linked },
                composing,
                look,
            )
        }))
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

/// One line of the window: its cells grouped into runs of the same look, with `composing` drawn just before the cursor, drawn as `look` says.
fn line_of(
    cells: &[Cell],
    cursor: Option<usize>,
    Marks { marked, linked }: Marks,
    composing: &str,
    look: Look,
) -> Node {
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
    let mut row = node().styles([flex(), flex_none(), h_px(look.font * LEADING)]);
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
            Some((held, content)) if *held == shared && !shared.cursor => content.push(character),
            _ => {
                if let Some((held, content)) = run.take() {
                    row = row.child(run_node(held, content, look));
                }
                if shared.cursor && !composing.is_empty() {
                    row = row.child(composing_node(composing, look.font));
                }
                run = Some((shared, character.to_string()));
            }
        }
    }
    if let Some((held, content)) = run {
        row = row.child(run_node(held, content, look));
    }
    row
}

/// The node drawing `composing`, text an input method has not committed yet, underlined in the default colours, `font` logical pixels tall.
fn composing_node(composing: &str, font: f32) -> Node {
    text(composing).styles([
        flex_none(),
        code_px(font),
        leading_ratio(LEADING),
        font_mono(),
        text_color(Role::Text),
        underline(),
    ])
}

/// How a paint is shown: a colour, or the default one of `role`.
fn shown(paint: Paint, role: Role) -> Color {
    match paint {
        Paint::Default => role.into(),
        Paint::Indexed(index) => {
            let (red, green, blue) = pm_terminal::indexed(index);
            Color::rgb(red, green, blue)
        }
        Paint::Rgb(red, green, blue) => Color::rgb(red, green, blue),
    }
}

/// The node drawing `content` as the cells of `run` share, with type and cursor as `drawn` says.
fn run_node(look: Run, content: String, drawn: Look) -> Node {
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
    let mut styles = vec![
        flex_none(),
        code_px(drawn.font),
        leading_ratio(LEADING),
        font_mono(),
        text_color(ink),
    ];
    if let Some(ground) = ground {
        styles.push(bg(ground));
    }
    if look.flags.has(Flags::BOLD) {
        styles.push(font_bold());
    }
    if look.flags.has(Flags::ITALIC) {
        styles.push(italic());
    }
    if look.flags.has(Flags::UNDERLINE) || look.linked {
        styles.push(underline());
    }
    if look.flags.has(Flags::STRIKE) {
        styles.push(line_through());
    }
    if look.cursor {
        match drawn.cursor {
            TerminalCursor::Block => styles.extend([
                bg(Role::Subtle),
                text_color(Role::Background),
                bg(Role::Text).focus(),
            ]),
            TerminalCursor::Bar => styles.extend([
                border_width(Sides::Left, 2.0),
                border_color(Role::Subtle),
                border_color(Role::Text).focus(),
            ]),
            TerminalCursor::Underline => styles.extend([
                border_width(Sides::Bottom, 2.0),
                border_color(Role::Subtle),
                border_color(Role::Text).focus(),
            ]),
        }
        return text(content).styles(styles).focused_with(TERMINAL_BODY);
    }
    text(content).styles(styles)
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
