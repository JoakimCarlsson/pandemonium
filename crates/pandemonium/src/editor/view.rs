//! The pane that draws one buffer: its gutter, its text and its cursor.
//!
//! The pane is a window onto the document rather than a rendering of it:
//! only the lines it has room for are measured, highlighted and drawn, so a
//! file of a hundred thousand lines costs a frame what a file of fifty does.
//! What the pane learns while drawing — how many lines fit, how wide a
//! character came out, where the text begins — it writes back into the
//! document as a [`TextLayout`], because that is the one measurement a click
//! arriving later has to agree with.

use std::ops::Range;
use std::sync::Arc;

use pm_core::{Blame, Change, ChangeKind};
use pm_gfx::{FontStyle, Point, Quad, Rect, Rgba, Size};
use pm_text::{Buffer, Diagnostic, Highlight, Highlights, Position, Selection, Severity};
use pm_ui::{
    Element, Glyphs, IconName, IconSize, LayoutContext, PaintContext, PointerCursor, ResizeEvent,
    ResizePhase, Style, Styled, Theme,
};

use crate::editor::OpenFile;
use crate::editor::display::{CursorShape, Display};
use crate::editor::layout::{GUTTER_GAP, GUTTER_INSET, TextLayout};
use crate::editor::minimap::{MINIMAP_WIDTH, Minimap};
use crate::editor::search::Search;

/// Width of the cursor while the pane is focused.
const CURSOR_WIDTH: f32 = 2.0;

/// How strongly a block caret covers the character under it, which has to
/// stay legible through it.
const BLOCK_ALPHA: f32 = 0.6;

/// Thickness of the line under a diagnostic.
const SQUIGGLE_WIDTH: f32 = 1.5;

/// Thickness of the line under a name the link key has turned into one.
const LINK_WIDTH: f32 = 1.0;

/// Width of the bar saying how far through the file the pane is looking.
const SCROLLBAR_WIDTH: f32 = 6.0;

/// How far that bar sits from the edges of the pane.
const SCROLLBAR_PADDING: f32 = 4.0;

/// Shortest the bar's thumb is drawn, however long the file is.
const SCROLLBAR_MIN_THUMB: f32 = 25.0;

/// Longest a selection may be and still light up where else it appears.
const OCCURRENCE_LIMIT: usize = 64;

/// How many columns past the end of a line its code lenses begin.
const LENS_GAP: usize = 2;

/// What stands between two code lenses on one line.
const LENS_SEPARATOR: &str = " | ";

/// Most lines kept in sight at the top of a pane while their body scrolls.
const STICKY_LIMIT: usize = 4;

/// Width of the bar marking a line that differs from the index.
const CHANGE_WIDTH: f32 = 3.0;

/// How tall a mark on the scrollbar's track is drawn.
const MARKER_HEIGHT: f32 = 2.0;

/// How far past the scrollbar a mark on its track reaches.
const MARKER_REACH: f32 = 5.0;

/// How tall the mark for lines taken out is drawn.
const REMOVED_HEIGHT: f32 = 3.0;

/// What a drag on a scrollbar reports, given the axis and the scale of it.
type ScrollHandler<M> = Arc<dyn Fn(ScrollAxis, ResizeEvent, f32) -> M>;

/// Narrowest a pane is and still gives room to a minimap.
const MINIMAP_ROOM: f32 = 480.0;

/// What a gesture over the text reports: its stage, and the places it spans.
type SelectHandler<M> = Arc<dyn Fn(ResizePhase, Position, Position) -> M>;

/// How wide a breakpoint's dot is, as a share of a line's height.
const BREAKPOINT_SIZE: f32 = 0.55;

/// How strongly the line a paused program stands on is washed.
const STOPPED_ALPHA: f32 = 0.18;

/// How strongly the breakpoint the pointer would set is drawn.
const BREAKPOINT_HINT_ALPHA: f32 = 0.35;

/// How thick the ring of a breakpoint the debugger could not place is.
const BREAKPOINT_RING: f32 = 1.5;

/// One breakpoint, as the gutter marks it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Breakpoint {
    /// The line it is on, counted from zero.
    pub line: usize,
    /// Whether the debugger could put it there, or has not been asked yet.
    pub placed: bool,
}

/// How much there is to scroll through, and how far in the view has reached.
#[derive(Clone, Copy)]
struct Reach {
    /// How much there is in all, in lines or in columns.
    total: usize,
    /// How much of it the pane is showing.
    showing: usize,
    /// How far in the first of what is showing sits.
    at: usize,
}

/// Which way a drag on a scrollbar moves the view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollAxis {
    /// Down the file.
    Vertical,
    /// Along its lines.
    Horizontal,
}

/// A pane showing one open file.
pub struct BufferView<M> {
    /// The document being drawn, which is told how much room it has.
    file: OpenFile,
    /// Whether keystrokes are going to this pane.
    focused: bool,
    /// What a press or a drag over the text sends, given where it reached.
    on_select: Option<SelectHandler<M>>,
    /// What a press or a drag down the gutter sends, given the lines it spans.
    on_gutter: Option<Arc<dyn Fn(Position, Position) -> M>>,
    /// What dragging a scrollbar sends, given the drag and the scale of it.
    on_scroll: Option<ScrollHandler<M>>,
    /// What a press in the fold column sends, given the line it landed on.
    on_fold: Option<Arc<dyn Fn(Position) -> M>>,
    /// What a press in the breakpoint column sends, given the line it
    /// landed on.
    on_breakpoint: Option<Arc<dyn Fn(Position) -> M>>,
    /// The breakpoints of the file, to mark in the gutter.
    breakpoints: Vec<Breakpoint>,
    /// The line a paused program stands on in this file, if it does.
    stopped: Option<usize>,
    /// What a press or a drag on the minimap sends, given the line it is on.
    on_minimap: Option<Arc<dyn Fn(usize) -> M>>,
    /// What a press of the secondary button over the pane sends.
    on_menu: Option<M>,
    /// The name the pointer is over, while the key that links it is held.
    link: Option<Range<Position>>,
    /// The name the editor is saying something about, while it says it.
    hovered: Option<Range<Position>>,
    /// The matches of modal editing's search, lit whether or not the search
    /// bar is open.
    found: Vec<Range<Position>>,
    /// Whether the caret is solid this instant, for its blink.
    caret: bool,
    /// Whether the pane is the text and nothing else.
    ///
    /// A commit message is edited in the same editor a file is, but none of
    /// what surrounds a file belongs around it: it has no line numbers to
    /// give, nothing to fold, nothing to blame and nowhere to scroll to.
    plain: bool,
    /// Which of the things drawn around the text it draws.
    display: Display,
    /// How the pane is sized within its parent.
    style: Style,
}

/// A pane showing `file`, focused or not.
pub fn buffer_view<M>(file: OpenFile, focused: bool) -> BufferView<M> {
    BufferView {
        file,
        focused,
        on_select: None,
        on_gutter: None,
        on_fold: None,
        on_breakpoint: None,
        breakpoints: Vec::new(),
        stopped: None,
        on_scroll: None,
        on_minimap: None,
        on_menu: None,
        link: None,
        hovered: None,
        found: Vec::new(),
        caret: true,
        plain: false,
        display: Display::default(),
        style: Style::default(),
    }
    .w_full()
    .flex_1()
}

/// A pane showing `file` as text alone, with nothing drawn around it.
pub fn plain_view<M>(file: OpenFile, focused: bool) -> BufferView<M> {
    BufferView {
        plain: true,
        ..buffer_view(file, focused)
    }
}

impl<M> BufferView<M> {
    /// Returns this pane placing the cursor and selecting through `on_select`.
    ///
    /// The handler is given the stage of the gesture, where it began and
    /// where it has reached, in lines and columns, because only the pane
    /// knows how wide a character came out and how far along the line it had
    /// scrolled.
    pub fn on_select(
        mut self,
        on_select: impl Fn(ResizePhase, Position, Position) -> M + 'static,
    ) -> Self {
        self.on_select = Some(Arc::new(on_select));
        self
    }

    /// Returns this pane selecting whole lines by drags down its gutter.
    pub fn on_gutter(mut self, on_gutter: impl Fn(Position, Position) -> M + 'static) -> Self {
        self.on_gutter = Some(Arc::new(on_gutter));
        self
    }

    /// Returns this pane folding and unfolding through `on_fold`.
    pub fn on_fold(mut self, on_fold: impl Fn(Position) -> M + 'static) -> Self {
        self.on_fold = Some(Arc::new(on_fold));
        self
    }

    /// Returns this pane setting and clearing breakpoints through
    /// `on_breakpoint`, by presses left of the line numbers.
    pub fn on_breakpoint(mut self, on_breakpoint: impl Fn(Position) -> M + 'static) -> Self {
        self.on_breakpoint = Some(Arc::new(on_breakpoint));
        self
    }

    /// Returns this pane marking `breakpoints` in its gutter.
    pub fn breakpoints(mut self, breakpoints: Vec<Breakpoint>) -> Self {
        self.breakpoints = breakpoints;
        self
    }

    /// Returns this pane marking `line` as where a paused program stands.
    pub fn stopped(mut self, line: Option<usize>) -> Self {
        self.stopped = line;
        self
    }

    /// Returns this pane with scrollbars that report drags through `on_scroll`.
    pub fn on_scroll(
        mut self,
        on_scroll: impl Fn(ScrollAxis, ResizeEvent, f32) -> M + 'static,
    ) -> Self {
        self.on_scroll = Some(Arc::new(on_scroll));
        self
    }

    /// Returns this pane scrolling to the line a press on its minimap lands
    /// on, through `on_minimap`.
    pub fn on_minimap(mut self, on_minimap: impl Fn(usize) -> M + 'static) -> Self {
        self.on_minimap = Some(Arc::new(on_minimap));
        self
    }

    /// Returns this pane opening a menu with `message` on the other button.
    pub fn on_menu(mut self, message: M) -> Self {
        self.on_menu = Some(message);
        self
    }

    /// Returns this pane drawing `span` as the link the pointer is over.
    ///
    /// Holding the key that follows a name to where it is defined turns the
    /// name under the pointer into a link, the way it does everywhere else:
    /// underlined, and under a pointer that says it can be pressed.
    pub fn link(mut self, span: Option<Range<Position>>) -> Self {
        self.link = span;
        self
    }

    /// Lights `found`, the matches of modal editing's search.
    pub fn found(mut self, found: Vec<Range<Position>>) -> Self {
        self.found = found;
        self
    }

    /// Returns this pane lighting up `span`, the name being talked about.
    ///
    /// What a hover is about is shown on the text as well as beside it, so
    /// that a panel which opened over a crowded line still says which name
    /// it answered for.
    pub fn hovered(mut self, span: Option<Range<Position>>) -> Self {
        self.hovered = span;
        self
    }

    /// Returns this pane drawing its caret solid or through a blink.
    pub fn caret(mut self, solid: bool) -> Self {
        self.caret = solid;
        self
    }

    /// Returns this pane drawing what `display` asks for around its text.
    pub fn display(mut self, display: Display) -> Self {
        self.display = display;
        self
    }
}

impl<M> Styled for BufferView<M> {
    /// How the pane is sized within its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: Clone + 'static> Element<M> for BufferView<M> {
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
        let font = cx.theme().text.code;
        let cell = Size::new(cx.measure("M", font).width.max(1.0), font.line_height);
        let theme = *cx.theme();

        let file = self.file.clone();
        let mut document = file.borrow_mut();
        let count = document.buffer().line_count();
        let blame = TextLayout::blame_for(document.is_blamed(), cell);
        let gutter = match self.plain {
            true => TextLayout::plain_gutter(),
            false => TextLayout::gutter_for(count, cell, self.display.line_numbers) + blame,
        };

        let minimap = match !self.plain && self.display.minimap && bounds.size.width > MINIMAP_ROOM
        {
            true => MINIMAP_WIDTH,
            false => 0.0,
        };
        let sizing = TextLayout {
            bounds,
            cell,
            gutter,
            blame,
            minimap,
            first: document.scroll(),
            offset: document.offset(),
            column: document.column(),
        };
        document.follow_cursor(sizing.rows(), sizing.columns());

        let layout = TextLayout {
            first: document.scroll(),
            offset: document.offset(),
            column: document.column(),
            ..sizing
        };
        document.set_layout(layout);

        let rows = layout.rows();
        let drawn = document.drawn_lines(layout.first, layout.drawn_rows());
        let span = *drawn.first().unwrap_or(&0)..drawn.last().map_or(0, |last| last + 1);
        let folded = drawn
            .iter()
            .map(|line| document.is_folded_at(*line))
            .collect::<Vec<_>>();
        let changes = document.changes().to_vec();
        let highlights = document.buffer_mut().highlights(span.clone());
        let painting = Painting {
            layout,
            font,
            theme: &theme,
            buffer: document.buffer(),
            search: document.search(),
            selection: document.buffer().selection(),
            selections: document.buffer().selections(),
            highlights: &highlights,
            brackets: document.buffer().matching_bracket(),
            occurrences: occurrences(document.buffer(), span.clone()),
            changes: changes.clone(),
            blame: document.blame(),
            drawn,
            folded,
            hovered: cx.input().pointer.filter(|at| layout.over_folds(*at)),
            link: self.link.clone(),
            talked_about: self.hovered.clone(),
        };

        cx.push_clip(bounds);
        if !self.plain && self.display.current_line {
            self.paint_current_line(&painting, cx);
        }
        self.paint_stopped(&painting, cx);
        self.paint_search(&painting, cx);
        if self.display.occurrences {
            self.paint_occurrences(&painting, cx);
        }
        self.paint_talked_about(&painting, cx);
        if !self.plain && self.display.indent_guides {
            self.paint_guides(&painting, cx);
        }
        if !self.plain {
            self.paint_wrap_guide(&painting, cx);
        }

        let mut glyphs = Glyphs::default();
        for line in painting.drawn.clone() {
            self.paint_line(line, &painting, &mut glyphs, cx);
        }
        self.paint_brackets(&painting, cx);
        self.paint_link(&painting, cx);
        self.paint_cursor(&painting, cx);
        if !self.plain {
            self.paint_changes(&painting, cx);
            self.paint_breakpoints(&painting, cx);
            self.paint_blame(&painting, &mut glyphs, cx);
            self.paint_folds(&painting, cx);
            if self.display.sticky_scroll {
                self.paint_sticky(&painting, &mut glyphs, cx);
            }
        }
        cx.pop_clip();

        let widest = painting.buffer.widest(span);
        let marks = markers(&painting);
        let strip = (minimap > 0.0).then(|| Minimap::of(bounds, count, layout.first, rows));
        if let Some(strip) = strip {
            let view = layout.first..layout.first + rows;
            let lit = cx.input().is_over(strip.area);
            strip.paint(document.buffer_mut(), &changes, view, lit, &theme, cx);
        }
        drop(document);

        self.select_region(layout, cx);
        if !self.plain {
            self.gutter_region(layout, cx);
            self.fold_region(layout, cx);
            self.breakpoint_region(layout, cx);
            if let Some(strip) = strip {
                self.minimap_region(strip, cx);
            }
            if self.display.scrollbars {
                self.paint_scrollbars(layout, count, widest, &marks, &theme, cx);
            }
        }
    }
}

impl<M> BufferView<M> {
    /// Marks the line the cursor is on, when nothing is selected.
    fn paint_current_line(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let (layout, selection) = (painting.layout, painting.selection);
        if !selection.is_empty() || !self.focused || self.display.whole_lines {
            return;
        }
        let Some(top) = painting.top_of(selection.head.line) else {
            return;
        };
        cx.quad(Quad::filled(
            Rect::from_xywh(
                layout.bounds.left(),
                top,
                layout.bounds.size.width,
                layout.cell.height,
            ),
            painting
                .theme
                .colors
                .text
                .alpha(painting.theme.emphasis.current_line),
        ));
    }

    /// Washes the line a paused program stands on.
    fn paint_stopped(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let Some(top) = self.stopped.and_then(|line| painting.top_of(line)) else {
            return;
        };
        let layout = painting.layout;
        cx.quad(Quad::filled(
            Rect::from_xywh(
                layout.bounds.left(),
                top,
                layout.bounds.size.width,
                layout.cell.height,
            ),
            painting.theme.colors.warning.alpha(STOPPED_ALPHA),
        ));
    }

    /// Marks the breakpoints left of the numbers, the line a paused program
    /// stands on, and — faintly — the line the pointer would set one on.
    fn paint_breakpoints(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        if self.on_breakpoint.is_none() {
            return;
        }
        let layout = painting.layout;
        let column = layout.breakpoint_column();
        let size = (layout.cell.height * BREAKPOINT_SIZE).min(column.size.width);
        let dot = |top: f32| {
            Rect::from_xywh(
                column.left() + (column.size.width - size) / 2.0,
                top + (layout.cell.height - size) / 2.0,
                size,
                size,
            )
        };
        let danger = painting.theme.colors.danger;

        let hovered = cx
            .input()
            .pointer
            .filter(|at| column.contains(*at))
            .and_then(|at| painting.drawn.get(layout.row_at(at)).copied())
            .filter(|line| !self.breakpoints.iter().any(|mark| mark.line == *line));
        if let Some(top) = hovered.and_then(|line| painting.top_of(line)) {
            cx.quad(
                Quad::filled(dot(top), danger.alpha(BREAKPOINT_HINT_ALPHA))
                    .corner_radius(size / 2.0),
            );
        }
        for mark in &self.breakpoints {
            let Some(top) = painting.top_of(mark.line) else {
                continue;
            };
            let quad = match mark.placed {
                true => Quad::filled(dot(top), danger),
                false => Quad::filled(dot(top), Rgba::TRANSPARENT).border(BREAKPOINT_RING, danger),
            };
            cx.quad(quad.corner_radius(size / 2.0));
        }
        if let Some(top) = self.stopped.and_then(|line| painting.top_of(line)) {
            let icon = IconSize::XSmall.pixels();
            let bounds = Rect::from_xywh(
                column.left() + (column.size.width - icon) / 2.0,
                top + (layout.cell.height - icon) / 2.0,
                icon,
                icon,
            );
            cx.icon(
                bounds,
                IconName::ArrowRight.svg(),
                painting.theme.colors.warning,
            );
        }
    }

    /// Lights up every place the selected word also appears on screen.
    fn paint_occurrences(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let color = painting
            .theme
            .colors
            .text
            .alpha(painting.theme.emphasis.occurrence);
        for found in &painting.occurrences {
            self.wash(found.clone(), color, painting, cx);
        }
    }

    /// Lights up the name the editor is saying something about.
    fn paint_talked_about(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let Some(span) = painting.talked_about.clone() else {
            return;
        };
        self.wash(span, painting.theme.colors.surface_hover, painting, cx);
    }

    /// Lights up every match of what is being looked for on screen.
    fn paint_search(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let found = painting
            .theme
            .colors
            .warning
            .alpha(painting.theme.emphasis.search);
        for span in &self.found {
            self.wash(span.clone(), found, painting, cx);
        }
        if !painting.search.is_open() {
            return;
        }
        for found in painting.search.matches() {
            let strength = if painting.search.is_current(found) {
                painting.theme.emphasis.search_current
            } else {
                painting.theme.emphasis.search
            };
            let color = painting.theme.colors.warning.alpha(strength);
            self.wash(found.clone(), color, painting, cx);
        }
    }

    /// Draws a line at every step of indentation the text stands at.
    fn paint_guides(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let layout = painting.layout;
        let color = painting
            .theme
            .colors
            .border_variant
            .alpha(painting.theme.emphasis.guide);

        let step = painting.buffer.indent().width.max(1);
        cx.push_clip(layout.text_area());
        for line in painting.drawn.iter().copied() {
            let Some(top) = painting.top_of(line) else {
                continue;
            };
            let indent = painting
                .buffer
                .line_chars(line)
                .take_while(|ch| ch.is_whitespace())
                .count();
            if indent == 0 || indent == painting.buffer.line_len(line) {
                continue;
            }
            let width = painting.column_of(Position::new(line, indent));
            for step in (step..width).step_by(step) {
                cx.quad(Quad::filled(
                    Rect::from_xywh(layout.x_of(step), top, 1.0, layout.cell.height),
                    color,
                ));
            }
        }
        cx.pop_clip();
    }

    /// Draws a line down the column the reader keeps lines short of.
    fn paint_wrap_guide(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let Some(column) = self.display.wrap_guide else {
            return;
        };
        let layout = painting.layout;
        let area = layout.text_area();
        cx.push_clip(area);
        cx.quad(Quad::filled(
            Rect::from_xywh(layout.x_of(column), area.top(), 1.0, area.size.height),
            painting.theme.colors.border_variant,
        ));
        cx.pop_clip();
    }

    /// Outlines the bracket at the cursor and the one that answers it.
    fn paint_brackets(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let Some((here, there)) = painting.brackets else {
            return;
        };
        let color = painting
            .theme
            .colors
            .cursor
            .alpha(painting.theme.emphasis.bracket);
        for at in [here, there] {
            let span = at..Position::new(at.line, at.column + 1);
            self.wash(span, color, painting, cx);
        }
    }

    /// Fills what `range` covers, however many lines it spans.
    fn wash(
        &self,
        range: Range<Position>,
        color: Rgba,
        painting: &Painting<'_>,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let layout = painting.layout;
        for line in range.start.line..=range.end.line {
            let Some(top) = painting.top_of(line) else {
                continue;
            };
            let from = if line == range.start.line {
                painting.column_of(range.start)
            } else {
                0
            };
            let to = if line == range.end.line {
                painting.column_of(range.end)
            } else {
                painting.buffer.display_width(line) + 1
            };
            if to <= from {
                continue;
            }
            cx.quad(Quad::filled(
                Rect::from_xywh(
                    layout.x_of(from),
                    top,
                    (to - from) as f32 * layout.cell.width,
                    layout.cell.height,
                ),
                color,
            ));
        }
    }

    /// Draws one line: its number, its selection, its text and its faults.
    fn paint_line(
        &self,
        line: usize,
        painting: &Painting<'_>,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let layout = painting.layout;
        let Some(top) = painting.top_of(line) else {
            return;
        };
        let buffer = painting.buffer;

        self.paint_selection(line, painting, cx);
        if !self.plain && self.display.line_numbers {
            self.paint_number(line, painting, glyphs, cx);
        }

        cx.push_clip(layout.text_area());
        let mut column = 0;
        let mut hints = buffer.hints_on(line).peekable();
        for (index, ch) in buffer.line_chars(line).enumerate() {
            while let Some(hint) = hints.next_if(|hint| hint.position.column == index) {
                column = self.paint_hint(hint, column, top, painting, glyphs, cx);
            }
            let width = if ch == '\t' {
                painting.buffer.tab_width() - column % painting.buffer.tab_width()
            } else {
                1
            };
            let drawn = column;
            column += width;
            if ch.is_whitespace() {
                continue;
            }
            let x = layout.x_of(drawn);
            if x + layout.cell.width < layout.text_left() {
                continue;
            }
            if x > layout.bounds.right() {
                break;
            }
            let color = match painting.highlights.at(line, index) {
                Some(highlight) => tint(highlight, painting.theme),
                None => painting.theme.colors.text,
            };
            let run = glyphs.shape(ch, painting.font, cx);
            cx.text(Point::new(x, top), run, color);
        }

        for hint in hints {
            column = self.paint_hint(hint, column, top, painting, glyphs, cx);
        }
        let lenses = buffer.lenses_on(line).collect::<Vec<_>>();
        if !lenses.is_empty() {
            let said = lenses.join(LENS_SEPARATOR);
            self.paint_note(&said, column + LENS_GAP, top, painting, glyphs, cx);
        }
        for diagnostic in buffer.diagnostics() {
            self.paint_diagnostic(diagnostic, line, top, painting, cx);
        }
        cx.pop_clip();
    }

    /// Writes one hint into a line, and says which column the line reaches.
    fn paint_hint(
        &self,
        hint: &pm_text::Hint,
        column: usize,
        top: f32,
        painting: &Painting<'_>,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) -> usize {
        self.paint_note(&hint.text, column, top, painting, glyphs, cx);
        column + hint.width()
    }

    /// Writes what a server said, not the file, into a line from `column` on.
    fn paint_note(
        &self,
        text: &str,
        column: usize,
        top: f32,
        painting: &Painting<'_>,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let layout = painting.layout;
        for (index, ch) in text.chars().enumerate() {
            if ch.is_whitespace() {
                continue;
            }
            let x = layout.x_of(column + index);
            if x > layout.bounds.right() {
                break;
            }
            let run = glyphs.shape(ch, painting.font, cx);
            cx.text(Point::new(x, top), run, painting.theme.colors.text_subtle);
        }
    }

    /// Fills what every cursor has selected on one line.
    fn paint_selection(
        &self,
        line: usize,
        painting: &Painting<'_>,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let color = painting
            .theme
            .colors
            .selection
            .alpha(painting.theme.emphasis.selection);

        for selection in &painting.selections {
            if !selection.touches(line) || (selection.is_empty() && !self.display.whole_lines) {
                continue;
            }
            let (start, end) = match self.display.whole_lines {
                true => (
                    Position::new(selection.start().line, 0),
                    Position::new(
                        selection.end().line,
                        painting.buffer.line_len(selection.end().line) + 1,
                    ),
                ),
                false => (selection.start(), selection.end()),
            };
            let from = if start.line == line {
                start
            } else {
                Position::new(line, 0)
            };
            let to = if end.line == line {
                end
            } else {
                Position::new(line, painting.buffer.line_len(line) + 1)
            };
            self.wash(from..to, color, painting, cx);
        }
    }

    /// Draws one line's number in the gutter.
    fn paint_number(
        &self,
        line: usize,
        painting: &Painting<'_>,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let layout = painting.layout;
        let Some(top) = painting.top_of(line) else {
            return;
        };
        let head = painting.selection.head.line;
        let color = if line == head {
            painting.theme.colors.text_muted
        } else {
            painting.theme.colors.text_subtle
        };
        let number = match self.display.relative_line_numbers && line != head {
            true => line.abs_diff(head).to_string(),
            false => (line + 1).to_string(),
        };
        let right = layout.blame_left() - GUTTER_GAP;

        for (index, digit) in number.chars().rev().enumerate() {
            let run = glyphs.shape(digit, painting.font, cx);
            let x = right - (index + 1) as f32 * layout.cell.width;
            cx.text(Point::new(x, top), run, color);
        }
    }

    /// Keeps the lines the top of the pane is inside in sight above it.
    ///
    /// What is drawn over are the first lines of the body of those very
    /// blocks, which have already been read: the trade is a few lines of
    /// what you are in the middle of for knowing what you are in the middle
    /// of at all. It is drawn in a layer of its own, because within one
    /// layer every quad is drawn before every glyph — a panel painted late
    /// without one would still sit under the text it is covering.
    fn paint_sticky(
        &self,
        painting: &Painting<'_>,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let layout = painting.layout;
        let Some(first) = painting.drawn.first().copied() else {
            return;
        };
        let holders = painting
            .buffer
            .enclosing(first)
            .into_iter()
            .filter(|line| *line < first)
            .collect::<Vec<_>>();
        let holders = &holders[holders.len().saturating_sub(STICKY_LIMIT)..];
        if holders.is_empty() {
            return;
        }

        cx.push_layer();
        cx.quad(Quad::filled(
            Rect::from_xywh(
                layout.bounds.left(),
                layout.bounds.top(),
                layout.bounds.size.width,
                holders.len() as f32 * layout.cell.height,
            ),
            painting.theme.colors.surface,
        ));

        for (row, line) in holders.iter().copied().enumerate() {
            let top = layout.bounds.top() + row as f32 * layout.cell.height;
            self.paint_pinned(line, top, painting, glyphs, cx);
        }
        cx.quad(Quad::filled(
            Rect::from_xywh(
                layout.bounds.left(),
                layout.bounds.top() + holders.len() as f32 * layout.cell.height,
                layout.bounds.size.width,
                1.0,
            ),
            painting.theme.colors.border,
        ));
        cx.pop_layer();
    }

    /// Writes one pinned line where the text of the pane would have been.
    fn paint_pinned(
        &self,
        line: usize,
        top: f32,
        painting: &Painting<'_>,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let layout = painting.layout;
        let number = (line + 1).to_string();
        let right = layout.blame_left() - GUTTER_GAP;
        let digits = number.chars().rev().filter(|_| self.display.line_numbers);
        for (index, digit) in digits.enumerate() {
            let run = glyphs.shape(digit, painting.font, cx);
            let x = right - (index + 1) as f32 * layout.cell.width;
            cx.text(Point::new(x, top), run, painting.theme.colors.text_subtle);
        }

        cx.push_clip(layout.text_area());
        let mut column = 0;
        for ch in painting.buffer.line_chars(line) {
            let width = if ch == '\t' {
                painting.buffer.tab_width() - column % painting.buffer.tab_width()
            } else {
                1
            };
            let drawn = column;
            column += width;
            if ch.is_whitespace() {
                continue;
            }
            let run = glyphs.shape(ch, painting.font, cx);
            cx.text(
                Point::new(layout.x_of(drawn), top),
                run,
                painting.theme.colors.text_muted,
            );
        }
        cx.pop_clip();
    }

    /// Marks the lines that hold something folded away, or that could.
    ///
    /// A closed fold is always marked, because a line is missing under it; a
    /// line that could be folded is marked only while the pointer is in the
    /// column, the way every editor does it — a chevron beside every
    /// indented line is a column of chevrons.
    fn paint_folds(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let layout = painting.layout;
        let hovered = painting.hovered_line();
        let size = IconSize::XSmall.pixels();

        for (row, line) in painting.drawn.iter().copied().enumerate() {
            let Some(top) = layout.top_of(row) else {
                continue;
            };
            let closed = painting.folded.get(row).copied().unwrap_or_default();
            if !closed && (hovered != Some(line) || !painting.buffer.is_foldable(line)) {
                continue;
            }
            let (glyph, color) = if closed {
                (IconName::ChevronRight, painting.theme.colors.text_muted)
            } else {
                (IconName::ChevronDown, painting.theme.colors.text_subtle)
            };
            let bounds = Rect::from_xywh(
                layout.fold_left(),
                top + (layout.cell.height - size) / 2.0,
                size,
                size,
            );
            cx.icon(bounds, glyph.svg(), color);
        }
    }

    /// Marks the lines that differ from what the index holds.
    fn paint_changes(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let layout = painting.layout;
        for change in &painting.changes {
            let color = match change.kind {
                ChangeKind::Added => painting.theme.colors.success,
                ChangeKind::Modified => painting.theme.colors.warning,
                ChangeKind::Removed => painting.theme.colors.danger,
            };
            if change.kind == ChangeKind::Removed {
                let Some(top) = painting.top_of(change.anchor()) else {
                    continue;
                };
                cx.quad(Quad::filled(
                    Rect::from_xywh(
                        layout.bounds.left(),
                        top + layout.cell.height - REMOVED_HEIGHT / 2.0,
                        CHANGE_WIDTH,
                        REMOVED_HEIGHT,
                    ),
                    color,
                ));
                continue;
            }
            for line in change.lines.clone() {
                let Some(top) = painting.top_of(line) else {
                    continue;
                };
                cx.quad(Quad::filled(
                    Rect::from_xywh(layout.bounds.left(), top, CHANGE_WIDTH, layout.cell.height),
                    color,
                ));
            }
        }
    }

    /// Writes who last changed each line, in the column before the text.
    ///
    /// A run of lines from one commit is named once, at the first of them
    /// that is on screen: what the column is for is seeing where the
    /// authorship changes, and repeating the same name down twenty lines
    /// hides exactly that.
    fn paint_blame(
        &self,
        painting: &Painting<'_>,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let layout = painting.layout;
        if layout.blame <= 0.0 {
            return;
        }
        let width = super::layout::BLAME_WIDTH;
        let mut above = None;

        for line in painting.drawn.iter().copied() {
            let Some(top) = painting.top_of(line) else {
                continue;
            };
            let Some(blame) = painting.blame.get(line) else {
                continue;
            };
            let repeated = above == Some(blame);
            above = Some(blame);
            if repeated {
                continue;
            }
            let label = if blame.uncommitted {
                "Uncommitted".to_owned()
            } else {
                format!("{} · {}", blame.author, blame.when)
            };
            let color = if line == painting.selection.head.line {
                painting.theme.colors.text_muted
            } else {
                painting.theme.colors.text_subtle
            };

            for (index, ch) in label.chars().take(width).enumerate() {
                if ch.is_whitespace() {
                    continue;
                }
                let run = glyphs.shape(ch, painting.font, cx);
                let x = layout.blame_left() + index as f32 * layout.cell.width;
                cx.text(Point::new(x, top), run, color);
            }
        }
    }

    /// Underlines what a diagnostic covers on one line, and marks the gutter.
    fn paint_diagnostic(
        &self,
        diagnostic: &Diagnostic,
        line: usize,
        top: f32,
        painting: &Painting<'_>,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let layout = painting.layout;
        let Some(columns) = diagnostic.columns(line, painting.buffer.line_len(line)) else {
            return;
        };
        let start = painting.column_of(Position::new(line, columns.start));
        let end = painting.column_of(Position::new(line, columns.end));
        let color = severity(diagnostic.severity, painting.theme);
        cx.quad(Quad::filled(
            Rect::from_xywh(
                layout.x_of(start),
                top + layout.cell.height - SQUIGGLE_WIDTH * 2.0,
                end.saturating_sub(start).max(1) as f32 * layout.cell.width,
                SQUIGGLE_WIDTH,
            ),
            color,
        ));
        cx.quad(Quad::filled(
            Rect::from_xywh(
                layout.bounds.left() + GUTTER_INSET / 2.0 + CHANGE_WIDTH,
                top + layout.cell.height / 2.0 - SQUIGGLE_WIDTH,
                SQUIGGLE_WIDTH * 2.0,
                SQUIGGLE_WIDTH * 2.0,
            ),
            color,
        ));
    }

    /// Draws the line under the name the link key has turned into one.
    fn paint_link(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        let Some(span) = painting.link.clone() else {
            return;
        };
        let Some(top) = painting.top_of(span.start.line) else {
            return;
        };
        let layout = painting.layout;
        let left = layout.x_of(painting.column_of(span.start));
        let right = layout.x_of(painting.column_of(span.end));

        cx.quad(Quad::filled(
            Rect::from_xywh(
                left,
                top + layout.cell.height - LINK_WIDTH,
                (right - left).max(0.0),
                LINK_WIDTH,
            ),
            painting.theme.colors.link,
        ));
    }

    /// The character the cursor of `selection` is drawn over.
    ///
    /// A block stands on a character rather than between two, so at the far
    /// end of a selection running forward it stands on the last character
    /// selected rather than on the one after it. A selection of whole lines
    /// has its ends where the cursors are, so it needs no such step back.
    fn cursor_cell(&self, selection: &Selection) -> Position {
        let head = selection.head;
        match self.display.cursor_shape == CursorShape::Block
            && !self.display.whole_lines
            && head > selection.anchor
            && head.column > 0
        {
            true => Position::new(head.line, head.column - 1),
            false => head,
        }
    }

    /// Draws every cursor: solid while the pane is focused, faint otherwise.
    fn paint_cursor(&self, painting: &Painting<'_>, cx: &mut PaintContext<'_, '_, M>) {
        if !self.caret {
            return;
        }
        let layout = painting.layout;
        let color = if self.focused {
            painting.theme.colors.cursor
        } else {
            painting.theme.colors.text_subtle
        };

        for selection in &painting.selections {
            let head = self.cursor_cell(selection);
            let Some(top) = painting.top_of(head.line) else {
                continue;
            };
            let (x, cell) = (layout.x_of(painting.column_of(head)), layout.cell);
            let (rect, color) = match self.display.cursor_shape {
                CursorShape::Bar => (Rect::from_xywh(x, top, CURSOR_WIDTH, cell.height), color),
                CursorShape::Block => (
                    Rect::from_xywh(x, top, cell.width, cell.height),
                    color.alpha(BLOCK_ALPHA),
                ),
                CursorShape::Underline => (
                    Rect::from_xywh(
                        x,
                        top + cell.height - CURSOR_WIDTH,
                        cell.width,
                        CURSOR_WIDTH,
                    ),
                    color,
                ),
            };
            cx.quad(Quad::filled(rect, color));
        }
    }
}

impl<M: Clone + 'static> BufferView<M> {
    /// Takes the press and the drag that place the cursor and select text.
    fn select_region(&mut self, layout: TextLayout, cx: &mut PaintContext<'_, '_, M>) {
        let Some(on_select) = self.on_select.clone() else {
            return;
        };
        let file = self.file.clone();
        let cursor = match self.link.is_some() {
            true => PointerCursor::Pointer,
            false => PointerCursor::Text,
        };
        cx.draggable(
            layout.text_area(),
            cursor,
            Arc::new(move |event| {
                let document = file.borrow();
                on_select(
                    event.phase,
                    document.position_at(event.start),
                    document.position_at(event.current),
                )
            }),
            self.on_menu.clone(),
        );
    }

    /// Takes the press and the drag down the gutter that select whole lines.
    fn gutter_region(&mut self, layout: TextLayout, cx: &mut PaintContext<'_, '_, M>) {
        let Some(on_gutter) = self.on_gutter.clone() else {
            return;
        };
        let gutter = Rect::from_xywh(
            layout.bounds.left(),
            layout.bounds.top(),
            layout.gutter,
            layout.bounds.size.height,
        );
        let file = self.file.clone();
        cx.draggable(
            gutter,
            PointerCursor::Default,
            Arc::new(move |event| {
                let document = file.borrow();
                on_gutter(
                    document.position_at(event.start),
                    document.position_at(event.current),
                )
            }),
            self.on_menu.clone(),
        );
    }

    /// Takes the press in the fold column that opens and closes a fold.
    ///
    /// It is registered after the gutter it sits inside, so it is the region
    /// the pointer finds first: the last one painted is the one on top.
    fn fold_region(&mut self, layout: TextLayout, cx: &mut PaintContext<'_, '_, M>) {
        let Some(on_fold) = self.on_fold.clone() else {
            return;
        };
        let column = Rect::from_xywh(
            layout.fold_left(),
            layout.bounds.top(),
            super::layout::FOLD_WIDTH,
            layout.bounds.size.height,
        );
        let file = self.file.clone();
        let pointer = cx.input().pointer;
        let pressed = pointer.map(|at| on_fold(file.borrow().position_at(at)));
        cx.clickable(column, pressed, self.on_menu.clone());
    }

    /// Takes the press in the breakpoint column that sets or clears one.
    ///
    /// It is registered after the gutter it sits inside, so a press there
    /// sets a breakpoint rather than selecting the line.
    fn breakpoint_region(&mut self, layout: TextLayout, cx: &mut PaintContext<'_, '_, M>) {
        let Some(on_breakpoint) = self.on_breakpoint.clone() else {
            return;
        };
        let file = self.file.clone();
        let pointer = cx.input().pointer;
        let pressed = pointer.map(|at| on_breakpoint(file.borrow().position_at(at)));
        cx.clickable(layout.breakpoint_column(), pressed, self.on_menu.clone());
    }

    /// Takes the press and the drag on the minimap that move the view.
    fn minimap_region(&mut self, strip: Minimap, cx: &mut PaintContext<'_, '_, M>) {
        let Some(on_minimap) = self.on_minimap.clone() else {
            return;
        };
        cx.draggable(
            strip.area,
            PointerCursor::Default,
            Arc::new(move |event| on_minimap(strip.line_at(event.current.y))),
            None,
        );
    }

    /// Draws the scrollbars, and takes the drags on them the caller asked for.
    fn paint_scrollbars(
        &mut self,
        layout: TextLayout,
        count: usize,
        widest: usize,
        marks: &[(usize, Rgba)],
        theme: &Theme,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let bounds = layout.bounds;
        let rows = layout.rows();
        if count > rows {
            let track = Rect::from_xywh(
                bounds.right() - SCROLLBAR_PADDING - SCROLLBAR_WIDTH,
                bounds.top() + SCROLLBAR_PADDING,
                SCROLLBAR_WIDTH,
                (bounds.size.height - SCROLLBAR_PADDING * 2.0).max(0.0),
            );
            self.paint_markers(track, count, marks, cx);
            let reach = Reach {
                total: count,
                showing: rows,
                at: layout.first,
            };
            self.paint_thumb(ScrollAxis::Vertical, track, reach, theme, cx);
        }

        let columns = layout.columns();
        if widest > columns {
            let track = Rect::from_xywh(
                layout.text_left() + SCROLLBAR_PADDING,
                bounds.bottom() - SCROLLBAR_PADDING - SCROLLBAR_WIDTH,
                (layout.text_area().right() - layout.text_left() - SCROLLBAR_PADDING * 2.0)
                    .max(0.0),
                SCROLLBAR_WIDTH,
            );
            let reach = Reach {
                total: widest,
                showing: columns,
                at: layout.column,
            };
            self.paint_thumb(ScrollAxis::Horizontal, track, reach, theme, cx);
        }
    }

    /// Marks the whole file's faults, changes and matches along the track.
    ///
    /// The track is the one place the whole file is in view at once, so what
    /// is worth going to is drawn on it: a scrollbar that only says where
    /// you are wastes the one view of the file that is always there.
    fn paint_markers(
        &mut self,
        track: Rect,
        count: usize,
        marks: &[(usize, Rgba)],
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        for (line, color) in marks {
            let along = track.size.height * *line as f32 / count.max(1) as f32;
            cx.quad(Quad::filled(
                Rect::from_xywh(
                    track.left() - MARKER_REACH,
                    track.top() + along,
                    track.size.width + MARKER_REACH,
                    MARKER_HEIGHT,
                ),
                *color,
            ));
        }
    }

    /// Draws one scrollbar's thumb and takes the drag along it.
    ///
    /// A bar down the side and a bar along the bottom are one control read
    /// on two axes: how much there is, how much of it is showing, and how
    /// far in the view has reached.
    fn paint_thumb(
        &mut self,
        axis: ScrollAxis,
        track: Rect,
        reach: Reach,
        theme: &Theme,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let Reach { total, showing, at } = reach;
        let vertical = axis == ScrollAxis::Vertical;
        let length = if vertical {
            track.size.height
        } else {
            track.size.width
        };
        let hidden = total.saturating_sub(showing).max(1) as f32;
        let extent = (length * showing as f32 / total as f32).max(SCROLLBAR_MIN_THUMB);
        let travel = (length - extent).max(0.0);
        let offset = travel * (at as f32 / hidden).min(1.0);
        let thumb = if vertical {
            Rect::from_xywh(track.left(), track.top() + offset, track.size.width, extent)
        } else {
            Rect::from_xywh(
                track.left() + offset,
                track.top(),
                extent,
                track.size.height,
            )
        };

        let interaction = match self.on_scroll.clone() {
            Some(on_scroll) => {
                let step = if travel > 0.0 { hidden / travel } else { 1.0 };
                cx.draggable(
                    thumb,
                    PointerCursor::Default,
                    Arc::new(move |event| on_scroll(axis, event, step)),
                    None,
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

/// What every line of one frame is drawn against.
struct Painting<'a> {
    /// Where the text sits and what one character comes to.
    layout: TextLayout,
    /// The type the buffer is set in.
    font: FontStyle,
    /// The tokens the frame is drawn from.
    theme: &'a Theme,
    /// The text being drawn.
    buffer: &'a Buffer,
    /// What is being looked for in it, and where that was found.
    search: &'a Search,
    /// What the primary cursor has selected, and where it is.
    selection: Selection,
    /// Every cursor, the primary one included.
    selections: Vec<Selection>,
    /// The highlights of the lines being drawn.
    highlights: &'a Highlights,
    /// The bracket at the cursor and the one that answers it.
    brackets: Option<(Position, Position)>,
    /// Where else on screen what is selected appears.
    occurrences: Vec<Range<Position>>,
    /// Where the file differs from what the index holds.
    changes: Vec<Change>,
    /// Who last changed each line, when the blame column is being drawn.
    blame: &'a [Blame],
    /// The lines being drawn, in the order they are drawn.
    drawn: Vec<usize>,
    /// Whether each of them has a fold closed under it.
    folded: Vec<bool>,
    /// Where the pointer is over the fold column, when it is.
    hovered: Option<Point>,
    /// The name the pointer is over, while the key that links it is held.
    link: Option<Range<Position>>,
    /// The name the editor is saying something about, while it says it.
    talked_about: Option<Range<Position>>,
}

impl Painting<'_> {
    /// The top of `line`, when it is one of the lines being drawn.
    fn top_of(&self, line: usize) -> Option<f32> {
        let row = self.drawn.binary_search(&line).ok()?;
        self.layout.top_of(row)
    }

    /// The line the pointer is over, when it is over the lines at all.
    fn hovered_line(&self) -> Option<usize> {
        let at = self.hovered?;
        self.drawn.get(self.layout.row_at(at)).copied()
    }
}

impl Painting<'_> {
    /// The column `position` is drawn at, tabs and hints counted in.
    fn column_of(&self, position: Position) -> usize {
        self.buffer.display_column(position)
    }
}

/// The lines worth marking on the scrollbar's track, and what to mark them in.
///
/// A fault outranks a change and a change outranks a match, because a line
/// with an error on it is a line the reader wants to find first however many
/// other reasons there are to go there.
fn markers(painting: &Painting<'_>) -> Vec<(usize, Rgba)> {
    let colors = painting.theme.colors;
    let mut marks = Vec::new();

    for found in painting.search.matches() {
        marks.push((found.start.line, colors.warning));
    }
    for change in &painting.changes {
        marks.push((
            change.anchor(),
            match change.kind {
                ChangeKind::Added => colors.success,
                ChangeKind::Modified => colors.warning,
                ChangeKind::Removed => colors.danger,
            },
        ));
    }
    for found in painting.buffer.diagnostics() {
        marks.push((
            found.range.start.line,
            severity(found.severity, painting.theme),
        ));
    }
    marks
}

/// Where else in `lines` the selected text, or the symbol at the cursor, appears.
///
/// A word is lit where it appears again the moment it is selected, which is
/// how selecting an identifier answers "where else is this used" without
/// being asked. A selection that spans lines, or that is long enough to be
/// prose rather than a name, lights nothing. With nothing selected, what is
/// lit is where a language server said the symbol at the cursor is used,
/// which knows a shadowed name from its namesake where matching text cannot.
fn occurrences(buffer: &Buffer, lines: Range<usize>) -> Vec<Range<Position>> {
    let selection = buffer.selection();
    let (start, end) = (selection.start(), selection.end());
    if selection.is_empty() {
        return buffer
            .uses()
            .iter()
            .filter(|span| lines.contains(&span.start.line))
            .cloned()
            .collect();
    }
    if start.line != end.line || end.column - start.column > OCCURRENCE_LIMIT {
        return Vec::new();
    }
    let needle = buffer.text_in(start..end);
    if needle.trim().is_empty() {
        return Vec::new();
    }
    let width = needle.chars().count();

    let mut found = Vec::new();
    for line in lines {
        let text = buffer.line_text(line);
        for (byte, _) in text.match_indices(&needle) {
            let column = text[..byte].chars().count();
            if Position::new(line, column) == start {
                continue;
            }
            found.push(Position::new(line, column)..Position::new(line, column + width));
        }
    }
    found
}

/// The colour `highlight` is drawn in.
pub fn tint(highlight: Highlight, theme: &Theme) -> Rgba {
    match highlight {
        Highlight::Keyword => theme.syntax.keyword,
        Highlight::String => theme.syntax.string,
        Highlight::Function => theme.syntax.function,
        Highlight::Comment => theme.syntax.comment,
        Highlight::Number => theme.syntax.number,
        Highlight::Type => theme.syntax.type_name,
        Highlight::Punctuation => theme.syntax.punctuation,
        Highlight::Variable => theme.syntax.variable,
        Highlight::Property => theme.syntax.property,
        Highlight::Constant => theme.syntax.constant,
        Highlight::Operator => theme.syntax.operator,
        Highlight::Tag => theme.syntax.tag,
        Highlight::Attribute => theme.syntax.attribute,
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
