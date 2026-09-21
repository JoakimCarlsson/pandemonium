//! The pane that draws one terminal, and tells it how large it is.
//!
//! The grid is drawn a cell at a time from the view's own glyph cache: a
//! terminal produces an unbounded number of distinct lines, so shaping them
//! as runs would fill the text system's cache with strings that are never
//! seen twice, while the characters themselves are a hundred-odd shapes that
//! repeat all day.

use std::collections::HashMap;
use std::sync::Arc;

use pm_gfx::{FontStyle, Point, Quad, Rect, Rgba, ShapedRun, Size};
use pm_ui::{Axis, Element, LayoutContext, PaintContext, ResizeEvent, Style, Styled, Theme};
use pm_vt::{Attrs, Cell, Color, Terminal};

use crate::terminal::Shell;

/// The type the grid is drawn in.
const FONT: FontStyle = FontStyle::new(14.0).mono().line_height(14.0 * LEADING);

/// How far apart the rows sit, as a multiple of the type size.
const LEADING: f32 = 1.4;

/// The weight a bold cell is drawn at.
const BOLD: u16 = 700;

/// How much of its colour a dim cell keeps.
const DIM: f32 = 0.6;

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

/// How much of its colour the thumb carries when it is not being used.
const THUMB_STRENGTH: f32 = 0.35;

/// How much of its colour the thumb carries under the pointer.
const THUMB_STRENGTH_ACTIVE: f32 = 0.6;

/// A pane showing one terminal's grid.
pub struct TerminalView<M> {
    /// The terminal being drawn, resized to whatever the pane comes to.
    shell: Shell,
    /// Whether keystrokes are going to this terminal.
    focused: bool,
    /// What a click on the grid sends, taken when the region is registered.
    on_focus: Option<M>,
    /// What dragging the scrollbar sends, given the drag and the scale of it.
    on_scroll: Option<Arc<dyn Fn(ResizeEvent, f32) -> M>>,
    /// How the pane is sized within its parent.
    style: Style,
}

/// A pane showing `shell`, which sends `on_focus` when it is clicked.
pub fn terminal_view<M>(shell: Shell, focused: bool, on_focus: M) -> TerminalView<M> {
    TerminalView {
        shell,
        focused,
        on_focus: Some(on_focus),
        on_scroll: None,
        style: Style::default(),
    }
    .w_full()
    .flex_1()
}

impl<M> TerminalView<M> {
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

impl<M: 'static> Element<M> for TerminalView<M> {
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
        let cell = Size::new(cx.measure("M", FONT).width.max(1.0), FONT.line_height);
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

        if let Some(message) = self.on_focus.take() {
            cx.interactive(bounds, message);
        }

        let shell = self.shell.clone();
        let terminal = shell.borrow();
        let theme = *cx.theme();
        let mut glyphs = Glyphs::default();
        cx.push_clip(bounds);
        for row in 0..rows {
            self.paint_row(&terminal, row, metrics, &theme, &mut glyphs, cx);
        }
        self.paint_cursor(&terminal, metrics, &theme, cx);
        cx.pop_clip();

        drop(terminal);
        self.paint_scrollbar(bounds, cell, &theme, cx);
    }
}

impl<M> TerminalView<M> {
    /// Draws the backgrounds and then the characters of one row.
    fn paint_row(
        &self,
        terminal: &Terminal,
        row: usize,
        metrics: Metrics,
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

        for (col, content) in line.cells().iter().enumerate() {
            if content.is_spacer() || content.is_blank() {
                continue;
            }
            let (foreground, background) = content.attrs.colors();
            let color = if cursor == Some(col) {
                resolve(background, theme, theme.colors.background)
            } else {
                shade(resolve(foreground, theme, theme.colors.text), content.attrs)
            };
            let origin = metrics.cell_at(row, col).origin;
            let run = glyphs.shape(content.ch, content.attrs, cx);
            cx.text(origin, run, color);
            self.paint_lines(content.attrs, metrics, origin, color, cx);
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
            THUMB_STRENGTH_ACTIVE
        } else {
            THUMB_STRENGTH
        };
        cx.quad(
            Quad::filled(thumb, theme.colors.text_subtle.alpha(strength))
                .corner_radius(SCROLLBAR_WIDTH / 2.0),
        );
    }
}

/// The glyphs shaped so far this frame, one per character and style.
#[derive(Default)]
struct Glyphs {
    /// Runs already shaped, keyed by the character, its weight and its slant.
    runs: HashMap<(char, u16, bool), Arc<ShapedRun>>,
}

impl Glyphs {
    /// The shaped run for `ch` in `attrs`, shaping it the first time only.
    fn shape<M>(
        &mut self,
        ch: char,
        attrs: Attrs,
        cx: &mut PaintContext<'_, '_, M>,
    ) -> Arc<ShapedRun> {
        let weight = if attrs.bold { BOLD } else { FONT.weight };
        self.runs
            .entry((ch, weight, attrs.italic))
            .or_insert_with(|| {
                let mut font = FONT.weight(weight);
                if attrs.italic {
                    font = font.italic();
                }
                cx.shape(&ch.to_string(), font)
            })
            .clone()
    }
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
fn shade(color: Rgba, attrs: Attrs) -> Rgba {
    if attrs.hidden {
        return Rgba::TRANSPARENT;
    }
    if attrs.dim {
        return color.alpha(DIM);
    }
    color
}

/// An opaque colour from three 8-bit channels.
fn channels(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}
