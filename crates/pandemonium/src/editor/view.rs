//! The pane that draws one buffer: its gutter, its text and its cursor.
//!
//! The pane is a window onto the document rather than a rendering of it:
//! only the lines it has room for are measured, highlighted and drawn, so a
//! file of a hundred thousand lines costs a frame what a file of fifty does.
//! What the pane learns while drawing — how many lines fit, where the text
//! begins — it writes back into the document, because nothing else in the
//! window knows how tall a line came out.

use std::sync::Arc;

use pm_gfx::{FontStyle, Point, Quad, Rect, Rgba, Size};
use pm_text::{Diagnostic, Highlight, Highlights, Position, Selection, Severity};
use pm_ui::{
    Element, Glyphs, LayoutContext, PaintContext, PointerCursor, ResizeEvent, Style, Styled, Theme,
};

use crate::editor::OpenFile;

/// The type a buffer is drawn in.
const FONT: FontStyle = FontStyle::new(14.0).mono().line_height(14.0 * LEADING);

/// How far apart the lines sit, as a multiple of the type size.
const LEADING: f32 = 1.5;

/// How far the gutter's numbers sit from the text.
const GUTTER_GAP: f32 = 16.0;

/// How far the gutter's numbers sit from the edge of the pane.
const GUTTER_INSET: f32 = 12.0;

/// Shortest a line number column is, in digits.
const GUTTER_DIGITS: usize = 2;

/// Width of the cursor while the pane is focused.
const CURSOR_WIDTH: f32 = 2.0;

/// How much of its colour a selection carries.
const SELECTION_STRENGTH: f32 = 0.3;

/// How much of its colour the line the cursor is on carries.
const CURRENT_LINE_STRENGTH: f32 = 0.05;

/// Thickness of the line under a diagnostic.
const SQUIGGLE_WIDTH: f32 = 1.5;

/// Width of the bar saying how far down the file the pane is looking.
const SCROLLBAR_WIDTH: f32 = 6.0;

/// How far that bar sits from the edges of the pane.
const SCROLLBAR_PADDING: f32 = 4.0;

/// Shortest the bar's thumb is drawn, however long the file is.
const SCROLLBAR_MIN_THUMB: f32 = 25.0;

/// How much of its colour the thumb carries when it is not being used.
const THUMB_STRENGTH: f32 = 0.35;

/// How much of its colour the thumb carries under the pointer.
const THUMB_STRENGTH_ACTIVE: f32 = 0.6;

/// A pane showing one open file.
pub struct BufferView<M> {
    /// The document being drawn, which is told how much room it has.
    file: OpenFile,
    /// Whether keystrokes are going to this pane.
    focused: bool,
    /// What a press or a drag over the text sends, given where it reached.
    on_select: Option<Arc<dyn Fn(Position, Position) -> M>>,
    /// What dragging the scrollbar sends, given the drag and the scale of it.
    on_scroll: Option<Arc<dyn Fn(ResizeEvent, f32) -> M>>,
    /// How the pane is sized within its parent.
    style: Style,
}

/// A pane showing `file`, focused or not.
pub fn buffer_view<M>(file: OpenFile, focused: bool) -> BufferView<M> {
    BufferView {
        file,
        focused,
        on_select: None,
        on_scroll: None,
        style: Style::default(),
    }
    .w_full()
    .flex_1()
}

impl<M> BufferView<M> {
    /// Returns this pane placing the cursor and selecting through `on_select`.
    ///
    /// The handler is given where the gesture began and where it has
    /// reached, in lines and columns, because only the pane knows how wide a
    /// character came out.
    pub fn on_select(mut self, on_select: impl Fn(Position, Position) -> M + 'static) -> Self {
        self.on_select = Some(Arc::new(on_select));
        self
    }

    /// Returns this pane with a scrollbar that reports drags through `on_scroll`.
    pub fn on_scroll(mut self, on_scroll: impl Fn(ResizeEvent, f32) -> M + 'static) -> Self {
        self.on_scroll = Some(Arc::new(on_scroll));
        self
    }
}

impl<M> Styled for BufferView<M> {
    /// How the pane is sized within its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: 'static> Element<M> for BufferView<M> {
    /// How the pane is sized within its parent.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes everything it is offered; a buffer is as large as its pane.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        available
    }

    /// Tells the document how much room it has, then draws what fits.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let cell = Size::new(cx.measure("M", FONT).width.max(1.0), FONT.line_height);
        let rows = (bounds.size.height / cell.height).floor().max(1.0) as usize;

        let file = self.file.clone();
        let mut document = file.borrow_mut();
        document.set_rows(rows);

        let count = document.buffer().line_count();
        let first = document.scroll();
        let last = (first + rows).min(count);
        let metrics = Metrics {
            bounds,
            cell,
            gutter: GUTTER_INSET * 2.0 + digits(count) as f32 * cell.width + GUTTER_GAP,
            first,
        };

        let theme = *cx.theme();
        let selection = document.buffer().selection();
        let highlights = document.buffer_mut().highlights(first..last);
        let painting = Painting {
            metrics,
            theme: &theme,
            selection,
            highlights: &highlights,
        };

        cx.push_clip(bounds);
        self.paint_current_line(&painting, cx);
        let mut glyphs = Glyphs::default();
        for line in first..last {
            self.paint_line(&document, line, &painting, &mut glyphs, cx);
        }
        self.paint_cursor(&painting, cx);
        cx.pop_clip();

        drop(document);
        self.select_region(metrics, cx);
        self.paint_scrollbar(bounds, count, rows, first, &theme, cx);
    }
}

impl<M> BufferView<M> {
    /// Marks the line the cursor is on, when nothing is selected.
    fn paint_current_line(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let (metrics, selection) = (painting.metrics, painting.selection);
        if !selection.is_empty() || !self.focused {
            return;
        }
        let Some(top) = metrics.top_of(selection.head.line) else {
            return;
        };
        cx.quad(Quad::filled(
            Rect::from_xywh(
                metrics.bounds.left(),
                top,
                metrics.bounds.size.width,
                metrics.cell.height,
            ),
            painting.theme.colors.text.alpha(CURRENT_LINE_STRENGTH),
        ));
    }

    /// Draws one line: its number, its selection, its text and its faults.
    fn paint_line(
        &self,
        document: &crate::editor::Document,
        line: usize,
        painting: &Painting<'_>,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let metrics = painting.metrics;
        let Some(top) = metrics.top_of(line) else {
            return;
        };
        let buffer = document.buffer();
        let len = buffer.line_len(line);

        self.paint_selection(line, len, painting, cx);
        self.paint_number(line, painting, glyphs, cx);

        for (column, ch) in buffer.line_chars(line).enumerate() {
            if ch.is_whitespace() {
                continue;
            }
            let color = match painting.highlights.at(line, column) {
                Some(highlight) => tint(highlight, painting.theme),
                None => painting.theme.colors.text,
            };
            let run = glyphs.shape(ch, FONT, cx);
            cx.text(Point::new(metrics.column_at(column), top), run, color);
        }

        for diagnostic in buffer.diagnostics() {
            self.paint_diagnostic(diagnostic, line, len, top, painting, cx);
        }
    }

    /// Fills what the selection covers on one line.
    fn paint_selection(
        &self,
        line: usize,
        len: usize,
        painting: &Painting<'_>,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let (metrics, selection) = (painting.metrics, painting.selection);
        if selection.is_empty() || !selection.touches(line) {
            return;
        }
        let (start, end) = (selection.start(), selection.end());
        let from = if start.line == line { start.column } else { 0 };
        let to = if end.line == line {
            end.column
        } else {
            len + 1
        };
        let Some(top) = metrics.top_of(line) else {
            return;
        };

        cx.quad(Quad::filled(
            Rect::from_xywh(
                metrics.column_at(from),
                top,
                to.saturating_sub(from) as f32 * metrics.cell.width,
                metrics.cell.height,
            ),
            painting.theme.colors.accent.alpha(SELECTION_STRENGTH),
        ));
    }

    /// Draws one line's number in the gutter.
    fn paint_number(
        &self,
        line: usize,
        painting: &Painting<'_>,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let metrics = painting.metrics;
        let Some(top) = metrics.top_of(line) else {
            return;
        };
        let color = if line == painting.selection.head.line {
            painting.theme.colors.text_muted
        } else {
            painting.theme.colors.text_subtle
        };
        let number = (line + 1).to_string();
        let right = metrics.bounds.left() + metrics.gutter - GUTTER_GAP;

        for (index, digit) in number.chars().rev().enumerate() {
            let run = glyphs.shape(digit, FONT, cx);
            let x = right - (index + 1) as f32 * metrics.cell.width;
            cx.text(Point::new(x, top), run, color);
        }
    }

    /// Underlines what a diagnostic covers on one line, and marks the gutter.
    fn paint_diagnostic(
        &self,
        diagnostic: &Diagnostic,
        line: usize,
        len: usize,
        top: f32,
        painting: &Painting<'_>,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let metrics = painting.metrics;
        let Some(columns) = diagnostic.columns(line, len) else {
            return;
        };
        let color = severity(diagnostic.severity, painting.theme);
        cx.quad(Quad::filled(
            Rect::from_xywh(
                metrics.column_at(columns.start),
                top + metrics.cell.height - SQUIGGLE_WIDTH * 2.0,
                columns.len() as f32 * metrics.cell.width,
                SQUIGGLE_WIDTH,
            ),
            color,
        ));
        cx.quad(Quad::filled(
            Rect::from_xywh(
                metrics.bounds.left() + GUTTER_INSET / 2.0,
                top + metrics.cell.height / 2.0 - SQUIGGLE_WIDTH,
                SQUIGGLE_WIDTH * 2.0,
                SQUIGGLE_WIDTH * 2.0,
            ),
            color,
        ));
    }

    /// Draws the cursor: solid while the pane is focused, faint otherwise.
    fn paint_cursor(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let metrics = painting.metrics;
        let head = painting.selection.head;
        let Some(top) = metrics.top_of(head.line) else {
            return;
        };
        let color = if self.focused {
            painting.theme.colors.accent
        } else {
            painting.theme.colors.text_subtle
        };
        cx.quad(Quad::filled(
            Rect::from_xywh(
                metrics.column_at(head.column),
                top,
                CURSOR_WIDTH,
                metrics.cell.height,
            ),
            color,
        ));
    }
}

impl<M: 'static> BufferView<M> {
    /// Takes the press and the drag that place the cursor and select text.
    fn select_region(&mut self, metrics: Metrics, cx: &mut PaintContext<'_, '_, M>) {
        let Some(on_select) = self.on_select.clone() else {
            return;
        };
        let text = Rect::from_xywh(
            metrics.bounds.left() + metrics.gutter,
            metrics.bounds.top(),
            (metrics.bounds.size.width - metrics.gutter).max(0.0),
            metrics.bounds.size.height,
        );
        cx.draggable(
            text,
            PointerCursor::Text,
            Arc::new(move |event| {
                on_select(
                    metrics.position_of(event.start),
                    metrics.position_of(event.current),
                )
            }),
            None,
        );
    }

    /// Draws the scrollbar, and takes the drag on it the caller asked for.
    fn paint_scrollbar(
        &mut self,
        bounds: Rect,
        count: usize,
        rows: usize,
        first: usize,
        theme: &Theme,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        if count <= rows {
            return;
        }
        let track = Rect::from_xywh(
            bounds.right() - SCROLLBAR_PADDING - SCROLLBAR_WIDTH,
            bounds.top() + SCROLLBAR_PADDING,
            SCROLLBAR_WIDTH,
            (bounds.size.height - SCROLLBAR_PADDING * 2.0).max(0.0),
        );
        let hidden = (count - rows) as f32;
        let thumb_height =
            (track.size.height * rows as f32 / count as f32).max(SCROLLBAR_MIN_THUMB);
        let travel = (track.size.height - thumb_height).max(0.0);
        let thumb = Rect::from_xywh(
            track.left(),
            track.top() + travel * (first as f32 / hidden).min(1.0),
            track.size.width,
            thumb_height,
        );

        let interaction = match self.on_scroll.clone() {
            Some(on_scroll) => {
                let lines_per_pixel = if travel > 0.0 { hidden / travel } else { 1.0 };
                cx.draggable(
                    thumb,
                    PointerCursor::Default,
                    Arc::new(move |event| on_scroll(event, lines_per_pixel)),
                    None,
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

/// What every line of one frame is drawn against.
struct Painting<'a> {
    /// Where the text sits and what one character comes to.
    metrics: Metrics,
    /// The tokens the frame is drawn from.
    theme: &'a Theme,
    /// What is selected, and where the cursor is.
    selection: Selection,
    /// The highlights of the lines being drawn.
    highlights: &'a Highlights,
}

/// Where the text sits on the screen and what one character of it comes to.
#[derive(Clone, Copy)]
struct Metrics {
    /// The pane the buffer is drawn in.
    bounds: Rect,
    /// The extent of one character.
    cell: Size,
    /// How much of the pane the gutter takes on the left.
    gutter: f32,
    /// The first line the pane is showing.
    first: usize,
}

impl Metrics {
    /// The top of `line`, when the pane is showing it.
    fn top_of(&self, line: usize) -> Option<f32> {
        let row = line.checked_sub(self.first)?;
        Some(self.bounds.top() + row as f32 * self.cell.height)
    }

    /// The left edge of `column`.
    fn column_at(&self, column: usize) -> f32 {
        self.bounds.left() + self.gutter + column as f32 * self.cell.width
    }

    /// The place in the buffer `point` falls on.
    ///
    /// A point below the last line or right of the last character still
    /// names a place: the buffer clamps it to text that exists, which is
    /// what dragging past the end of a line is asking for.
    fn position_of(&self, point: Point) -> Position {
        let row = ((point.y - self.bounds.top()) / self.cell.height).floor();
        let column = ((point.x - self.bounds.left() - self.gutter) / self.cell.width).round();
        Position::new(self.first + row.max(0.0) as usize, column.max(0.0) as usize)
    }
}

/// How many digits it takes to write the number of `lines`.
fn digits(lines: usize) -> usize {
    lines.to_string().len().max(GUTTER_DIGITS)
}

/// The colour `highlight` is drawn in.
fn tint(highlight: Highlight, theme: &Theme) -> Rgba {
    match highlight {
        Highlight::Keyword => theme.syntax.keyword,
        Highlight::String => theme.syntax.string,
        Highlight::Function => theme.syntax.function,
        Highlight::Comment => theme.syntax.comment,
        Highlight::Number => theme.syntax.number,
        Highlight::Type => theme.syntax.type_name,
        Highlight::Punctuation => theme.syntax.punctuation,
    }
}

/// The colour a diagnostic of `severity` is marked in.
fn severity(severity: Severity, theme: &Theme) -> Rgba {
    match severity {
        Severity::Error => theme.colors.danger,
        Severity::Warning => theme.colors.warning,
        Severity::Information | Severity::Hint => theme.colors.accent,
    }
}
