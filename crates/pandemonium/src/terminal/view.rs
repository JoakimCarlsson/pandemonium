//! The pane that draws one terminal, and tells it how large it is.
//!
//! The grid is drawn a cell at a time from a [`Glyphs`] cache, for the
//! reason that cache exists: a terminal produces an unbounded number of
//! distinct lines, and the characters on them are a hundred-odd shapes that
//! repeat all day.

use std::sync::Arc;

use pm_gfx::{FontStyle, Point, Quad, Rect, Rgba, Size};
use pm_ui::{
    Axis, Element, Glyphs, LayoutContext, MenuItem, PaintContext, PointerCursor, ResizeEvent,
    ResizePhase, Style, Styled, Theme, menu_entry, menu_separator,
};
use pm_vt::{Attrs, Cell, Color, Link, Place, Terminal};

use crate::keymap::Action;
use crate::message::Message;
use crate::terminal::Shell;

/// The weight a bold cell is drawn at.
const BOLD: u16 = 700;

/// Thickness of the cursor's outline while the pane is not focused.
const CURSOR_OUTLINE: f32 = 1.0;

/// Thickness of an underline or a strikethrough.
const LINE_WIDTH: f32 = 1.0;

/// Width of the bar that says how far back through the scrollback the view is.
const SCROLLBAR_WIDTH: f32 = 6.0;

/// How far that bar sits from the edges of the pane.
const SCROLLBAR_PADDING: f32 = 4.0;

/// Shortest the bar's thumb is drawn, however long the scrollback is.
const SCROLLBAR_MIN_THUMB: f32 = 25.0;

/// What a press or a drag over the grid sends, given where it began and reached.
type PointHandler<M> = Arc<dyn Fn(ResizePhase, Place, Place) -> M>;

/// A pane showing one terminal's grid.
pub struct TerminalView<M> {
    /// The terminal being drawn, resized to whatever the pane comes to.
    shell: Shell,
    /// Whether keystrokes are going to this terminal.
    focused: bool,
    /// Whether the key that follows a link is held, making links pressable.
    linking: bool,
    /// What a press or a drag over the grid sends.
    on_point: Option<PointHandler<M>>,
    /// What a press of the secondary button over the grid sends.
    on_menu: Option<M>,
    /// What dragging the scrollbar sends, given the drag and the scale of it.
    on_scroll: Option<Arc<dyn Fn(ResizeEvent, f32) -> M>>,
    /// How the pane is sized within its parent.
    style: Style,
}

/// A pane showing `shell`, focused or not.
pub fn terminal_view<M>(shell: Shell, focused: bool) -> TerminalView<M> {
    TerminalView {
        shell,
        focused,
        linking: false,
        on_point: None,
        on_menu: None,
        on_scroll: None,
        style: Style::default(),
    }
    .w_full()
    .flex_1()
}

/// The things that can be done to what a terminal's screen shows.
///
/// Copy is offered only while something is picked out: copying nothing is a
/// command that silently empties the clipboard.
pub fn screen_menu(selected: bool) -> Vec<MenuItem<Message>> {
    let act = |action: Action| Message::ActOnTerminal(action);
    vec![
        menu_entry("Copy", selected.then_some(act(Action::Copy))),
        menu_entry("Paste", Some(act(Action::Paste))),
        menu_separator(),
        menu_entry("Select All", Some(act(Action::SelectAll))),
    ]
}

impl<M> TerminalView<M> {
    /// Returns this pane selecting and following links through `on_point`.
    ///
    /// The handler is given the stage of the gesture and the cells where it
    /// began and has reached, because only the pane knows how large a cell
    /// came out and how far back through the scrollback the view is.
    pub fn on_point(mut self, on_point: impl Fn(ResizePhase, Place, Place) -> M + 'static) -> Self {
        self.on_point = Some(Arc::new(on_point));
        self
    }

    /// Returns this pane opening a menu with `message` on the other button.
    pub fn on_menu(mut self, message: M) -> Self {
        self.on_menu = Some(message);
        self
    }

    /// Returns this pane with links pressable while `linking` holds.
    ///
    /// The link under the pointer is underlined either way; the pointer only
    /// says it can be pressed while the key that follows it is held.
    pub fn linking(mut self, linking: bool) -> Self {
        self.linking = linking;
        self
    }

    /// Returns this pane with a scrollbar that reports drags through `on_scroll`.
    ///
    /// The handler is given the drag and how many lines one pixel of it is
    /// worth, because only the pane knows how tall a row came out.
    pub fn on_scroll(mut self, on_scroll: impl Fn(ResizeEvent, f32) -> M + 'static) -> Self {
        self.on_scroll = Some(Arc::new(on_scroll));
        self
    }
}

impl<M> Styled for TerminalView<M> {
    /// How the pane is sized within its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: Clone + 'static> Element<M> for TerminalView<M> {
    /// How the pane is sized within its parent.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes everything it is offered; a terminal is as large as its pane.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        available
    }

    /// Resizes the terminal to the space it was given, then draws its grid.
    ///
    /// The grid is laid out from the bottom up: a pane rarely divides into a
    /// whole number of rows, and the leftover belongs above the text, so the
    /// line the shell is writing stays against the foot of the pane.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let font = cx.theme().text.terminal;
        let cell = Size::new(cx.measure("M", font).width.max(1.0), font.line_height);
        let cols = ((bounds.size.width - cell.width) / cell.width)
            .floor()
            .max(1.0) as usize;
        let rows = (bounds.size.height / cell.height).floor().max(1.0) as usize;
        let grid = Rect::from_xywh(
            bounds.left() + cell.width,
            bounds.bottom() - rows as f32 * cell.height,
            cols as f32 * cell.width,
            rows as f32 * cell.height,
        );
        let metrics = Metrics { bounds: grid, cell };
        self.shell.borrow_mut().resize(cols, rows);

        let shell = self.shell.clone();
        let terminal = shell.borrow();
        let theme = *cx.theme();
        let link = cx
            .input()
            .pointer
            .filter(|pointer| bounds.contains(*pointer))
            .and_then(|pointer| {
                let (row, col) = metrics.cell_under(pointer);
                terminal.link_at(terminal.place_at(row, col))
            });
        let screen = Screen {
            selection: terminal.selection_span(),
            link: link.as_ref(),
        };
        let mut glyphs = Glyphs::default();
        cx.push_clip(bounds);
        for row in 0..rows {
            self.paint_row(&terminal, row, metrics, &screen, &theme, &mut glyphs, cx);
        }
        self.paint_cursor(&terminal, metrics, &theme, cx);
        cx.pop_clip();
        drop(terminal);

        self.point_region(bounds, metrics, link.is_some(), cx);
        self.paint_scrollbar(bounds, cell, &theme, cx);
    }
}

impl<M: Clone + 'static> TerminalView<M> {
    /// Takes the press and the drag that select text and follow links.
    ///
    /// The whole pane answers, margins included: a drag that starts beside
    /// the first column is aimed at the first column.
    fn point_region(
        &mut self,
        bounds: Rect,
        metrics: Metrics,
        over_link: bool,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let Some(on_point) = self.on_point.clone() else {
            return;
        };
        let cursor = match over_link && self.linking {
            true => PointerCursor::Pointer,
            false => PointerCursor::Text,
        };
        let shell = self.shell.clone();
        cx.draggable(
            bounds,
            cursor,
            Arc::new(move |event| {
                let terminal = shell.borrow();
                let place = |point| {
                    let (row, col) = metrics.cell_under(point);
                    terminal.place_at(row, col)
                };
                on_point(event.phase, place(event.start), place(event.current))
            }),
            self.on_menu.clone(),
        );
    }
}

/// What is drawn over the grid's own cells: the selection and the link.
struct Screen<'a> {
    /// The first and last cells picked out, when anything is.
    selection: Option<(Place, Place)>,
    /// The link under the pointer, if it is over one.
    link: Option<&'a Link>,
}

impl Screen<'_> {
    /// Whether the cell at `place` is part of the link under the pointer.
    fn links(&self, place: Place) -> bool {
        self.link.is_some_and(|link| link.covers(place))
    }
}

impl<M> TerminalView<M> {
    /// Draws the backgrounds, the selection and then the characters of one row.
    #[allow(clippy::too_many_arguments)]
    fn paint_row(
        &self,
        terminal: &Terminal,
        row: usize,
        metrics: Metrics,
        screen: &Screen<'_>,
        theme: &Theme,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let Some(line) = terminal.grid().row(row) else {
            return;
        };
        let cursor = self
            .focused
            .then(|| terminal.grid().cursor_in_view())
            .flatten()
            .filter(|(cursor_row, _)| *cursor_row == row)
            .map(|(_, col)| col);

        self.paint_backgrounds(line.cells(), row, metrics, theme, cx);
        let line_number = terminal.place_at(row, 0).line;
        if let Some(columns) = selected_columns(screen.selection, line_number, line.cells().len()) {
            fill(cx, metrics, row, columns, theme.terminal.selection);
        }

        for (col, content) in line.cells().iter().enumerate() {
            if content.is_spacer() || content.is_blank() {
                continue;
            }
            let (foreground, background) = content.attrs.colors();
            let color = if cursor == Some(col) {
                resolve(background, theme, theme.colors.background)
            } else {
                shade(
                    resolve(foreground, theme, theme.colors.text),
                    content.attrs,
                    theme,
                )
            };
            let origin = metrics.cell_at(row, col).origin;
            let run = glyphs.shape(content.ch, font(content.attrs, theme), cx);
            cx.text(origin, run, color);
            let mut attrs = content.attrs;
            attrs.underline |= screen.links(Place::new(line_number, col));
            self.paint_lines(attrs, metrics, origin, color, cx);
        }
    }

    /// Fills the runs of cells whose background is not the pane's own.
    fn paint_backgrounds(
        &self,
        cells: &[Cell],
        row: usize,
        metrics: Metrics,
        theme: &Theme,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let mut run: Option<(usize, Rgba)> = None;
        for (col, content) in cells.iter().enumerate() {
            let (_, background) = content.attrs.colors();
            let color = match background {
                Color::Default if !content.attrs.inverse => Rgba::TRANSPARENT,
                color => resolve(color, theme, theme.colors.text),
            };
            match run {
                Some((_, filled)) if filled == color => continue,
                Some((start, filled)) => {
                    fill(cx, metrics, row, start..col, filled);
                    run = Some((col, color));
                }
                None => run = Some((col, color)),
            }
        }
        if let Some((start, filled)) = run {
            fill(cx, metrics, row, start..cells.len(), filled);
        }
    }

    /// Draws the underline and strikethrough a cell asks for.
    fn paint_lines(
        &self,
        attrs: Attrs,
        metrics: Metrics,
        origin: Point,
        color: Rgba,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let cell = metrics.cell;
        if attrs.underline {
            let y = origin.y + cell.height - LINE_WIDTH * 2.0;
            cx.quad(Quad::filled(
                Rect::from_xywh(origin.x, y, cell.width, LINE_WIDTH),
                color,
            ));
        }
        if attrs.strikethrough {
            let y = origin.y + cell.height / 2.0;
            cx.quad(Quad::filled(
                Rect::from_xywh(origin.x, y, cell.width, LINE_WIDTH),
                color,
            ));
        }
    }

    /// Draws the cursor: a block while the pane is focused, an outline otherwise.
    fn paint_cursor(
        &self,
        terminal: &Terminal,
        metrics: Metrics,
        theme: &Theme,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        if !terminal.modes().cursor_visible {
            return;
        }
        let Some((row, col)) = terminal.grid().cursor_in_view() else {
            return;
        };
        let at = metrics.cell_at(row, col);
        if self.focused {
            cx.quad(Quad::filled(at, theme.terminal.cursor));
        } else {
            cx.quad(
                Quad::filled(at, Rgba::TRANSPARENT).border(CURSOR_OUTLINE, theme.terminal.cursor),
            );
        }
    }
}

impl<M: 'static> TerminalView<M> {
    /// Draws the scrollbar, and takes the drag on it the caller asked for.
    ///
    /// There is nothing to draw while the whole of the output fits: a bar
    /// that always spans its track says nothing and takes a column of the
    /// pane to say it.
    fn paint_scrollbar(
        &mut self,
        bounds: Rect,
        cell: Size,
        theme: &Theme,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let terminal = self.shell.borrow();
        let grid = terminal.grid();
        let (history, rows, offset) = (grid.scrollback_len(), grid.rows(), grid.offset());
        drop(terminal);
        if history == 0 {
            return;
        }

        let track = Rect::from_xywh(
            bounds.right() - SCROLLBAR_PADDING - SCROLLBAR_WIDTH,
            bounds.top() + SCROLLBAR_PADDING,
            SCROLLBAR_WIDTH,
            (bounds.size.height - SCROLLBAR_PADDING * 2.0).max(0.0),
        );
        let total = (history + rows) as f32;
        let thumb_height = (track.size.height * rows as f32 / total).max(SCROLLBAR_MIN_THUMB);
        let travel = (track.size.height - thumb_height).max(0.0);
        let back = offset as f32 / history as f32;
        let thumb = Rect::from_xywh(
            track.left(),
            track.top() + travel * (1.0 - back),
            track.size.width,
            thumb_height,
        );

        let interaction = match self.on_scroll.clone() {
            Some(on_scroll) => {
                let lines_per_pixel = if travel > 0.0 {
                    history as f32 / travel
                } else {
                    cell.height
                };
                cx.resizable(
                    thumb,
                    Axis::Vertical,
                    Arc::new(move |event| on_scroll(event, lines_per_pixel)),
                )
            }
            None => Default::default(),
        };

        let strength = if interaction.hovered || interaction.pressed {
            theme.emphasis.scrollbar_active
        } else {
            theme.emphasis.scrollbar
        };
        cx.quad(
            Quad::filled(thumb, theme.colors.text_subtle.alpha(strength))
                .corner_radius(theme.radius.full),
        );
    }
}

/// The type a cell of `attrs` is drawn in, from `theme`'s terminal step.
fn font(attrs: Attrs, theme: &Theme) -> FontStyle {
    let grid = theme.text.terminal;
    let font = grid.weight(if attrs.bold { BOLD } else { grid.weight });
    if attrs.italic { font.italic() } else { font }
}

/// Where the grid sits on the screen and what one cell of it comes to.
#[derive(Clone, Copy)]
struct Metrics {
    /// The pane the grid is drawn in.
    bounds: Rect,
    /// The extent of one cell.
    cell: Size,
}

impl Metrics {
    /// The row and column of the cell under `point`, clamped to the grid.
    fn cell_under(&self, point: Point) -> (usize, usize) {
        let rows = (self.bounds.size.height / self.cell.height)
            .round()
            .max(1.0) as usize;
        let cols = (self.bounds.size.width / self.cell.width).round().max(1.0) as usize;
        let row = ((point.y - self.bounds.top()) / self.cell.height)
            .floor()
            .max(0.0) as usize;
        let col = ((point.x - self.bounds.left()) / self.cell.width)
            .floor()
            .max(0.0) as usize;
        (row.min(rows - 1), col.min(cols - 1))
    }

    /// The rectangle the cell at `row` and `col` occupies.
    fn cell_at(&self, row: usize, col: usize) -> Rect {
        Rect::from_xywh(
            self.bounds.left() + col as f32 * self.cell.width,
            self.bounds.top() + row as f32 * self.cell.height,
            self.cell.width,
            self.cell.height,
        )
    }
}

/// The columns of line `line` that `selection` picks out, `cols` wide.
fn selected_columns(
    selection: Option<(Place, Place)>,
    line: usize,
    cols: usize,
) -> Option<std::ops::Range<usize>> {
    let (start, end) = selection?;
    if line < start.line || line > end.line {
        return None;
    }
    let from = if line == start.line { start.col } else { 0 };
    let to = if line == end.line { end.col + 1 } else { cols };
    Some(from..to.min(cols))
}

/// Fills the `columns` of one row in `color`.
fn fill<M>(
    cx: &mut PaintContext<'_, '_, M>,
    metrics: Metrics,
    row: usize,
    columns: std::ops::Range<usize>,
    color: Rgba,
) {
    if color.is_transparent() || columns.is_empty() {
        return;
    }
    let first = metrics.cell_at(row, columns.start);
    cx.quad(Quad::filled(
        Rect::from_xywh(
            first.left(),
            first.top(),
            columns.len() as f32 * metrics.cell.width,
            metrics.cell.height,
        ),
        color,
    ));
}

/// The colour `color` comes to in `theme`, `default` standing for the pane's own.
fn resolve(color: Color, theme: &Theme, default: Rgba) -> Rgba {
    match color {
        Color::Default => default,
        Color::Indexed(index) if (index as usize) < theme.terminal.ansi.len() => {
            theme.terminal.ansi[index as usize]
        }
        Color::Indexed(index) => {
            let (r, g, b) = pm_vt::palette(index);
            channels(r, g, b)
        }
        Color::Rgb(r, g, b) => channels(r, g, b),
    }
}

/// The colour a dim or hidden cell is actually drawn in.
fn shade(color: Rgba, attrs: Attrs, theme: &Theme) -> Rgba {
    if attrs.hidden {
        return Rgba::TRANSPARENT;
    }
    if attrs.dim {
        return color.alpha(theme.emphasis.dim);
    }
    color
}

/// An opaque colour from three 8-bit channels.
fn channels(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}
