//! The terminal dock under the session: tabs of shells above the window of the one showing.

use crate::areas::DebugEvent;
use crate::areas::TerminalEvent;
use crate::events;
use crate::parts;
use crate::theme::Indicated;
use rustle::{Alignment, Arrangement, TextOverflow};

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::phosphor;

use std::hash::{Hash, Hasher};

use pm_config::TerminalCursor;
use pm_terminal::{Cell, Color as Paint, Emulator, Flags, Link};
use rustle::{Color, CursorIcon, Modifier};

use crate::Button;
use crate::dock::{BottomTab, Shared, TerminalTab};
use crate::events::Interactive;
use crate::parts::{glyph, line, spacer};
use crate::pointing::{Stroke, TerminalPointer};
use crate::sample::Session;
use crate::state::WorkspaceState;
use crate::theme::{Radius, Role};
use std::rc::Rc;

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
    let kind = cell_type(font);
    let size = rustle::layout_text(&"M".repeat(SAMPLE), &kind, None).size();
    (
        size.width / SAMPLE as f32,
        size.height.max(kind.line_height),
    )
}

/// The type the cells of a shell are set in: monospaced, `font` logical pixels tall, lines [`LEADING`] times that.
fn cell_type(font: f32) -> rustle::TextStyle {
    theme::code_style(font, LEADING)
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
    let height = if state.sizes.terminal_maximized.get() {
        (page * MAXIMIZED).max(state.sizes.terminal_height.get())
    } else {
        state.sizes.terminal_height.get()
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
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .height(height(state, page))
            .background(theme::color(Role::Background)),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::horizontal_divider(Modifier::new(), 1.0, theme::color(Role::Border));
            rustle::column_arranged(
                Modifier::new().fill_max_width().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    parts::bar(10, 2, 1, || {
                        pane_tab(
                            "Problems",
                            Some(errors + warnings).filter(|count| *count > 0),
                            shown == BottomTab::Problems,
                            Target::Terminal(TerminalEvent::BottomProblems),
                        );
                        if state.debug_docked() {
                            pane_tab(
                                "Debug Console",
                                None,
                                shown == BottomTab::Debug,
                                Target::Debug(DebugEvent::DebugTab),
                            );
                        }
                        pane_tab(
                            "Terminal",
                            None,
                            shown == BottomTab::Terminal,
                            Target::Terminal(TerminalEvent::BottomTerminal),
                        );
                        spacer();
                        if shown == BottomTab::Terminal {
                            Button::icon(
                                phosphor::PLUS,
                                Target::Terminal(TerminalEvent::NewTerminal),
                            )
                            .build();
                        }
                        Button::icon(
                            if state.sizes.terminal_maximized.get() {
                                phosphor::ARROWS_IN_SIMPLE
                            } else {
                                phosphor::ARROWS_OUT_SIMPLE
                            },
                            Target::Terminal(TerminalEvent::MaximizeTerminal),
                        )
                        .build();
                        Button::icon(phosphor::X, Target::Terminal(TerminalEvent::Terminal))
                            .build();
                    });
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
                                rustle::row_arranged(
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
        },
    );
}

/// The look of a tab or shell row: filled while it is the one `shown`, lit under the pointer otherwise.
fn tab_tones(shown: bool) -> theme::InteractionTones {
    if shown {
        theme::InteractionTones::new().idle(Role::SurfaceSelected)
    } else {
        theme::InteractionTones::new().hover(Role::SurfaceHover)
    }
}

/// Emits a tab of the dock named `label`, filled while it is the one showing, with how many things it holds when it holds any.
#[track_caller]
fn pane_tab(label: &str, count: Option<usize>, shown: bool, id: Target) {
    let tone = if shown { Role::Text } else { Role::Muted };
    let own = Modifier::new().height(theme::sp(8)).indicated(
        tab_tones(shown).radius(Radius::Lg),
        theme::Focused::Control(crate::focus::is_focused(id)),
    );
    rustle::row_arranged(
        events::pressed(own, id).padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            line(label, 13, 1.4, tone);
            if let Some(count) = count {
                rustle::row_arranged(
                    Modifier::new()
                        .background_rounded(theme::color(Role::Accent), Radius::Lg.px())
                        .padding_xy(theme::sp(1.5), 0.0),
                    Arrangement::Start,
                    Alignment::Center,
                    || line(count.to_string(), 11, 1.3, Role::OnAccent),
                );
            }
        },
    );
}

/// Emits the shells of `session` as a list at the right of the dock, the one showing marked, each with the button that ends it.
fn shell_list(session: &Session) {
    rustle::row_arranged(
        Modifier::new().fill_max_height().width(SHELL_LIST_WIDTH),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::vertical_divider(Modifier::new(), 1.0, theme::color(Role::Border));
            rustle::column_arranged(
                Modifier::new().weight(1.0).fill_max_height().clip(),
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
        },
    );
}

/// Emits the row of the shell at `index`: its name, pressed to show it, and the button that ends it.
fn shell_row(index: usize, tab: &TerminalTab, shown: bool) {
    let tone = if shown { Role::Text } else { Role::Muted };
    let select = Target::Terminal(TerminalEvent::TerminalTab(index));
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(7))
            .indicated(tab_tones(shown), theme::Focused::Rustle)
            .padding_each(0.0, 0.0, theme::sp(1), 0.0),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            let name = Modifier::new().weight(1.0).height(theme::sp(7)).clip();
            rustle::row_arranged(
                events::pressed(name, select).padding_each(theme::sp(3), 0.0, 0.0, 0.0),
                Arrangement::SpacedBy(theme::sp(2)),
                Alignment::Center,
                || {
                    glyph(phosphor::TERMINAL, 3.5, tone);
                    line(tab.name(), 13, 1.4, tone);
                },
            );
            Button::icon(
                phosphor::X,
                Target::Terminal(TerminalEvent::TerminalClose(index)),
            )
            .build();
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
    let body = Target::Terminal(TerminalEvent::TerminalBody);
    rustle::column_arranged(
        Modifier::new()
            .fill_max_height()
            .weight(1.0)
            .background(theme::color(Role::Background))
            .clip()
            .cursor(if link.is_some() {
                CursorIcon::Pointer
            } else {
                CursorIcon::Text
            })
            .chain(shell_pointer(&emulator, look.font))
            .pressable_pointing(
                body,
                send(Event::pressing(body)),
                {
                    let operations = crate::events::operations();
                    move |at| operations.open_terminal_menu(at)
                },
                shell_press(&emulator, look.font),
            )
            .chain(crate::bounds::placed(body, Modifier::new()))
            .chain(crate::bounds::sized(body, Modifier::new()))
            .chain(crate::focus::keyboard_focus(body))
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
    rustle::column_arranged(
        Modifier::new(),
        Arrangement::Start,
        Alignment::Start,
        || {
            for row in 0..emulator.rows() {
                let at_cursor = cursor
                    .filter(|(line, _)| *line == row)
                    .map(|(_, column)| column);
                let marked = selection.and_then(|range| marked_columns(range, first + row));
                let linked =
                    link.and_then(|link| marked_columns((link.start, link.end), first + row));
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
    rustle::row_arranged(
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
fn composing_node(composing: &str, font: f32) {
    rustle::basic_text(
        Modifier::new(),
        composing,
        cell_type(font).color(theme::color(Role::Text)).underline(),
        TextOverflow::Ellipsis,
    );
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

/// Emits `content` as the cells of `look` share, with type and cursor as `drawn` says; the cell under the cursor tells the window, through `ime_caret`, where an input method's candidates go while the shell has the keyboard.
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
        ink = ink.copy(ink.alpha() * 0.6);
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
        kind = kind.strikethrough();
    }
    let mut modifier = Modifier::new().fill_max_height();
    if let Some(ground) = ground {
        modifier = modifier.background(theme::color(ground));
    }
    if look.cursor {
        let caret = rustle::Rect::new(
            rustle::Offset::new(0.0, 0.0),
            rustle::Size::new(1.0, kind.line_height),
        );
        cursor_cell(modifier.ime_caret(caret), content, kind, ink, drawn.cursor);
    } else {
        rustle::basic_text(
            modifier,
            content,
            kind.color(theme::color(ink)),
            TextOverflow::Ellipsis,
        );
    }
}

/// Emits `content` in `kind` with `modifier` as the cell under the cursor, the cursor drawn as `shape` says: subtle, and in the text colour while the window holds the keyboard; its character in `ink`, or in the background colour on a block cursor.
#[track_caller]
fn cursor_cell(
    modifier: Modifier,
    content: String,
    kind: rustle::TextStyle,
    ink: Color,
    shape: TerminalCursor,
) {
    let cursor = if crate::focus::is_focused(Target::Terminal(TerminalEvent::TerminalBody)) {
        theme::color(Role::Text)
    } else {
        theme::color(Role::Subtle)
    };
    match shape {
        TerminalCursor::Block => rustle::basic_text(
            modifier.background(cursor),
            content,
            kind.color(theme::color(Role::Background)),
            TextOverflow::Ellipsis,
        ),
        TerminalCursor::Bar => {
            rustle::row_arranged(modifier, Arrangement::Start, Alignment::Start, || {
                rustle::vertical_divider(Modifier::new(), 2.0, cursor);
                rustle::basic_text(
                    Modifier::new().fill_max_height(),
                    content,
                    kind.color(theme::color(ink)),
                    TextOverflow::Ellipsis,
                );
            })
        }
        TerminalCursor::Underline => rustle::basic_text(
            modifier
                .border_bottom(2.0, cursor)
                .padding_each(0.0, 0.0, 0.0, 2.0),
            content,
            kind.color(theme::color(ink)),
            TextOverflow::Ellipsis,
        ),
    }
}

/// The cell of the window of the shell `emulator`, set in type `font` tall, that the point `at` from the top left of the window is over.
fn cell_of(emulator: &Shared, font: f32, at: rustle::Offset) -> (usize, usize) {
    let shown = emulator.read();
    cell_at(
        (at.x, at.y),
        cell_size(font),
        (shown.columns(), shown.rows()),
    )
}

/// What the window of the shell `emulator`, set in type `font` tall, does with its own
/// presses: the primary button going down tells the app the cell under it, how many presses
/// in a row it makes and the modifier keys held, which open a link or start marking text.
fn shell_press(emulator: &Shared, font: f32) -> impl Fn(&rustle::PointerEvent) + 'static {
    let operations = events::operations();
    let emulator = emulator.clone();
    move |event| {
        if event.button != rustle::PointerButton::Primary
            || event.phase != rustle::PointerPhase::Press
        {
            return;
        }
        operations.terminal_pointer(TerminalPointer {
            cell: cell_of(&emulator, font, event.position),
            stroke: Stroke::Down {
                clicks: event.click_count,
                modifiers: event.modifiers,
            },
        });
    }
}

/// The modifier of the window of the shell `emulator`, set in type `font` tall, that tells
/// the app the cell the pointer is over, the primary button pressed on it moving and let go
/// wherever the pointer is, and the wheel turned over it, which scrolls the shell back and
/// goes no further.
fn shell_pointer(emulator: &Shared, font: f32) -> Modifier {
    let operations = events::operations();
    let (hovered, held) = (Rc::clone(&operations), Rc::clone(&operations));
    let (over, pressed) = (emulator.clone(), emulator.clone());
    Modifier::new()
        .on_pointer_move(move |local| {
            hovered.terminal_hovered(local.map(|local| cell_of(&over, font, local)));
        })
        .pointer_input(move |event| {
            let stroke = match event.phase {
                rustle::PointerInputPhase::Move => Stroke::Move,
                rustle::PointerInputPhase::Up | rustle::PointerInputPhase::Cancel => Stroke::Up,
                rustle::PointerInputPhase::Scroll => {
                    held.terminal_wheel(-event.scroll_delta.y);
                    return true;
                }
                rustle::PointerInputPhase::Down => return false,
            };
            held.terminal_pointer(TerminalPointer {
                cell: cell_of(&pressed, font, event.position),
                stroke,
            });
            false
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
