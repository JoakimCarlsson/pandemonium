//! The pane the excerpts are read and edited in.
//!
//! Every row is one line tall, whatever it holds — a file's heading, a line
//! of it, a line the last commit had, the gap between two excerpts — so a
//! press is turned into a row by its height alone, and the rows are drawn
//! from where the pane is scrolled to and no further than it has room for.
//! Only the file holding the cursor draws one; the others are text to read
//! until a press puts the cursor in them.

use std::rc::Rc;
use std::sync::Arc;

use pm_core::ChangeKind;
use pm_gfx::{FontStyle, Point, Quad, Rect, Rgba, Size};
use pm_text::{Highlights, Position, Selection};
use pm_ui::{
    Element, Glyphs, LayoutContext, PaintContext, PointerCursor, ResizePhase, Style, Styled, Theme,
};

use crate::editor::{FileId, OpenFile, tint};
use crate::excerpts::store::{OpenExcerpts, Row};

/// How far the gutter's numbers sit from the edge of the pane.
const GUTTER_INSET: f32 = 12.0;

/// How far they sit from the text.
const GUTTER_GAP: f32 = 16.0;

/// Fewest digits the column of numbers is drawn wide enough for.
const DIGITS: usize = 3;

/// Width of the bar marking a line that differs from the last commit.
const CHANGE_WIDTH: f32 = 3.0;

/// Width of the caret.
const CURSOR_WIDTH: f32 = 2.0;

/// What a press or a drag over the text reports: its stage, the file it
/// began in and the places it spans there.
type SelectHandler<M> = Arc<dyn Fn(ResizePhase, FileId, Position, Position) -> M>;

/// Where each row drawn this frame came out, for a press to be read against.
struct Drawn {
    /// Top of the first row drawn.
    top: f32,
    /// Where the text begins, the gutter being to the left of it.
    left: f32,
    /// The extent of one character.
    cell: Size,
    /// For each row drawn, the file and the line it is, when it is a line.
    lines: Vec<Option<(usize, usize)>>,
    /// The files, by their place in the list.
    files: Vec<(FileId, OpenFile)>,
}

impl Drawn {
    /// The file and the place in it that `point` falls on.
    ///
    /// A press on a row that is not a line of the file — a heading, a line
    /// taken out, a gap — lands on the nearest line below it, or above it
    /// when there is none below.
    fn place(&self, point: Point) -> Option<(FileId, Position)> {
        let row = ((point.y - self.top) / self.cell.height.max(1.0))
            .floor()
            .max(0.0) as usize;
        let row = row.min(self.lines.len().saturating_sub(1));
        let (index, line) = self.lines[row..]
            .iter()
            .flatten()
            .next()
            .or_else(|| self.lines[..row].iter().rev().flatten().next())
            .copied()?;
        let (file, document) = self.files.get(index)?;
        let column = ((point.x - self.left) / self.cell.width.max(1.0))
            .round()
            .max(0.0) as usize;
        let buffer = document.borrow();
        let at = buffer.buffer().position_at_display(line, column);
        Some((*file, buffer.buffer().clamped(at)))
    }
}

/// The pane showing one worktree's excerpts.
pub struct ExcerptsView<M> {
    /// What the pane shows, which it tells how much room it had.
    excerpts: OpenExcerpts,
    /// Whether keystrokes are going to this pane.
    focused: bool,
    /// Whether the caret is solid this instant, for its blink.
    caret: bool,
    /// What a press or a drag over the text sends.
    on_select: Option<SelectHandler<M>>,
    /// What a press on a file's heading sends, given its place in the list.
    on_open: Option<Arc<dyn Fn(usize) -> M>>,
    /// How the pane is sized within its parent.
    style: Style,
}

/// The pane showing `excerpts`, focused or not.
pub fn excerpts_view<M>(excerpts: OpenExcerpts, focused: bool) -> ExcerptsView<M> {
    ExcerptsView {
        excerpts,
        focused,
        caret: true,
        on_select: None,
        on_open: None,
        style: Style::default(),
    }
    .w_full()
    .flex_1()
}

impl<M> ExcerptsView<M> {
    /// Returns this pane placing the cursor and selecting through `on_select`.
    pub fn on_select(
        mut self,
        on_select: impl Fn(ResizePhase, FileId, Position, Position) -> M + 'static,
    ) -> Self {
        self.on_select = Some(Arc::new(on_select));
        self
    }

    /// Returns this pane opening a file when its heading is pressed.
    pub fn on_open(mut self, on_open: impl Fn(usize) -> M + 'static) -> Self {
        self.on_open = Some(Arc::new(on_open));
        self
    }

    /// Returns this pane drawing its caret solid or through a blink.
    pub fn caret(mut self, solid: bool) -> Self {
        self.caret = solid;
        self
    }
}

impl<M> Styled for ExcerptsView<M> {
    /// How the pane is sized within its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

/// What every row of one frame is drawn against.
struct Painting<'a> {
    /// The tokens the frame is drawn from.
    theme: &'a Theme,
    /// The type the lines are set in.
    font: FontStyle,
    /// The extent of one character.
    cell: Size,
    /// Where the pane is.
    bounds: Rect,
    /// Where the text begins.
    left: f32,
    /// The files, by their place in the list.
    files: Vec<(FileId, OpenFile)>,
    /// The highlights of the lines drawn of each file.
    highlights: Vec<Highlights>,
    /// How each drawn line of each file differs from the last commit.
    kinds: Vec<Vec<(std::ops::Range<usize>, ChangeKind)>>,
    /// The file holding the cursor, by its place in the list.
    active: Option<usize>,
    /// Every cursor of that file.
    selections: Vec<Selection>,
}

impl<M: Clone + 'static> Element<M> for ExcerptsView<M> {
    /// How the pane is sized within its parent.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes everything it is offered.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        available
    }

    /// Brings the cursor into view, then draws the rows that fit.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let theme = *cx.theme();
        let font = theme.text.code;
        let cell = Size::new(cx.measure("M", font).width.max(1.0), font.line_height);
        let shown = (bounds.size.height / cell.height).floor().max(1.0) as usize;

        let mut excerpts = self.excerpts.borrow_mut();
        let rows = excerpts.rows();
        excerpts.follow(&rows, shown);
        let first = excerpts.scroll().min(rows.len().saturating_sub(1));
        let drawn = rows
            .iter()
            .skip(first)
            .take(shown)
            .cloned()
            .collect::<Vec<_>>();
        let digits = excerpts
            .files()
            .iter()
            .map(|excerpted| {
                excerpted
                    .document
                    .borrow()
                    .buffer()
                    .line_count()
                    .to_string()
                    .len()
            })
            .max()
            .unwrap_or(DIGITS)
            .max(DIGITS);
        let left = bounds.left() + GUTTER_INSET * 2.0 + digits as f32 * cell.width + GUTTER_GAP;
        let active = excerpts.active_index();
        let files = excerpts
            .files()
            .iter()
            .map(|excerpted| (excerpted.file, excerpted.document.clone()))
            .collect::<Vec<_>>();
        let kinds = excerpts
            .files_mut()
            .iter_mut()
            .map(|excerpted| {
                excerpted
                    .changes()
                    .iter()
                    .map(|change| (change.lines.clone(), change.kind))
                    .collect()
            })
            .collect();
        let counts = excerpts
            .files_mut()
            .iter_mut()
            .map(|excerpted| excerpted.counts())
            .collect::<Vec<_>>();
        let names = excerpts
            .files()
            .iter()
            .map(|excerpted| excerpted.name.clone())
            .collect::<Vec<_>>();
        drop(excerpts);

        let highlights = files
            .iter()
            .enumerate()
            .map(|(index, (_, document))| {
                let lines = drawn.iter().filter_map(|row| match row {
                    Row::Line(file, line) if *file == index => Some(*line),
                    _ => None,
                });
                let (low, high) = lines.fold((usize::MAX, 0), |(low, high), line| {
                    (low.min(line), high.max(line + 1))
                });
                match low < high {
                    true => document.borrow_mut().buffer_mut().highlights(low..high),
                    false => Highlights::default(),
                }
            })
            .collect();
        let selections = active
            .and_then(|index| files.get(index))
            .map(|(_, document)| document.borrow().buffer().selections())
            .unwrap_or_default();
        let painting = Painting {
            theme: &theme,
            font,
            cell,
            bounds,
            left,
            files,
            highlights,
            kinds,
            active,
            selections,
        };

        cx.push_clip(bounds);
        let mut glyphs = Glyphs::default();
        let mut caret = None;
        for (at, row) in drawn.iter().enumerate() {
            let top = bounds.top() + at as f32 * cell.height;
            match row {
                Row::Header(index) => {
                    self.paint_header(&painting, *index, &names[*index], counts[*index], top, cx);
                }
                Row::Line(index, line) => {
                    let drawn = self.paint_line(&painting, *index, *line, top, &mut glyphs, cx);
                    caret = caret.or(drawn);
                }
                Row::Removed(_, text) => self.paint_removed(&painting, text, top, &mut glyphs, cx),
                Row::Gap(_) => self.paint_gap(&painting, top, &mut glyphs, cx),
            }
        }
        cx.pop_clip();
        self.excerpts.borrow_mut().set_caret(caret);

        self.regions(&painting, &drawn, cx);
    }
}

impl<M: Clone + 'static> ExcerptsView<M> {
    /// Draws the heading of one file: its path, whether it has unsaved
    /// edits, and how much it adds and takes out.
    fn paint_header(
        &self,
        painting: &Painting<'_>,
        index: usize,
        name: &str,
        (added, removed): (usize, usize),
        top: f32,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let theme = painting.theme;
        let bounds = painting.bounds;
        let row = Rect::from_xywh(bounds.left(), top, bounds.size.width, painting.cell.height);
        cx.quad(Quad::filled(row, theme.colors.surface));
        cx.quad(Quad::filled(
            Rect::from_xywh(bounds.left(), top, bounds.size.width, 1.0),
            theme.colors.border_variant,
        ));

        let font = theme.text.sm.mono();
        let dirty = painting
            .files
            .get(index)
            .is_some_and(|(_, document)| document.borrow().buffer().is_dirty());
        let parts = [
            (name.to_owned(), theme.colors.text),
            (
                match dirty {
                    true => "●".to_owned(),
                    false => String::new(),
                },
                theme.colors.text_muted,
            ),
            (format!("+{added}"), theme.colors.success),
            (format!("−{removed}"), theme.colors.danger),
        ];
        let mut x = bounds.left() + GUTTER_INSET;
        let middle = top + (painting.cell.height - font.line_height) / 2.0;
        for (said, color) in parts {
            if said.is_empty() {
                continue;
            }
            let run = cx.shape(&said, font);
            let width = run.width;
            cx.text(Point::new(x, middle), run, color);
            x += width + GUTTER_GAP / 2.0;
        }
    }

    /// Draws one line of a file: its number, how it differs, what is
    /// selected on it, its text and its caret, answering just under where
    /// the primary cursor was drawn when it is on this line.
    fn paint_line(
        &self,
        painting: &Painting<'_>,
        index: usize,
        line: usize,
        top: f32,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) -> Option<Point> {
        let theme = painting.theme;
        let (bounds, cell) = (painting.bounds, painting.cell);
        let (_, document) = painting.files.get(index)?;
        let document = document.borrow();
        let buffer = document.buffer();

        let kind = painting.kinds[index]
            .iter()
            .find(|(lines, _)| lines.contains(&line))
            .map(|(_, kind)| *kind);
        if let Some(kind) = kind {
            let color = match kind {
                ChangeKind::Added => theme.colors.success,
                ChangeKind::Modified => theme.colors.warning,
                ChangeKind::Removed => theme.colors.danger,
            };
            cx.quad(Quad::filled(
                Rect::from_xywh(bounds.left(), top, bounds.size.width, cell.height),
                color.alpha(theme.emphasis.change),
            ));
            cx.quad(Quad::filled(
                Rect::from_xywh(bounds.left(), top, CHANGE_WIDTH, cell.height),
                color,
            ));
        }

        let active = painting.active == Some(index);
        if active {
            self.paint_selection(painting, buffer, line, top, cx);
        }

        let number = (line + 1).to_string();
        let right = painting.left - GUTTER_GAP;
        let color = match active && painting.selections.iter().any(|at| at.head.line == line) {
            true => theme.colors.text_muted,
            false => theme.colors.text_subtle,
        };
        for (at, digit) in number.chars().rev().enumerate() {
            let run = glyphs.shape(digit, painting.font, cx);
            cx.text(
                Point::new(right - (at + 1) as f32 * cell.width, top),
                run,
                color,
            );
        }

        let highlights = &painting.highlights[index];
        let tab = buffer.tab_width().max(1);
        let mut column = 0;
        for (at, ch) in buffer.line_chars(line).enumerate() {
            let width = match ch {
                '\t' => tab - column % tab,
                _ => 1,
            };
            let drawn = column;
            column += width;
            if ch.is_whitespace() {
                continue;
            }
            let x = painting.left + drawn as f32 * cell.width;
            if x > bounds.right() {
                break;
            }
            let color = highlights
                .at(line, at)
                .map_or(theme.colors.text, |highlight| tint(highlight, theme));
            let run = glyphs.shape(ch, painting.font, cx);
            cx.text(Point::new(x, top), run, color);
        }

        if active && self.caret {
            let color = match self.focused {
                true => theme.colors.cursor,
                false => theme.colors.text_subtle,
            };
            for selection in painting.selections.iter().filter(|at| at.head.line == line) {
                let x = painting.left + buffer.display_column(selection.head) as f32 * cell.width;
                cx.quad(Quad::filled(
                    Rect::from_xywh(x, top, CURSOR_WIDTH, cell.height),
                    color,
                ));
            }
        }
        let primary = buffer.selection().head;
        (active && primary.line == line).then(|| {
            Point::new(
                painting.left + buffer.display_column(primary) as f32 * cell.width,
                top + cell.height,
            )
        })
    }

    /// Fills what the cursors of the active file select on `line`.
    fn paint_selection(
        &self,
        painting: &Painting<'_>,
        buffer: &pm_text::Buffer,
        line: usize,
        top: f32,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let theme = painting.theme;
        let color = theme.colors.selection.alpha(theme.emphasis.selection);
        for selection in &painting.selections {
            if selection.is_empty() || !selection.touches(line) {
                continue;
            }
            let (start, end) = (selection.start(), selection.end());
            let from = match start.line == line {
                true => buffer.display_column(start),
                false => 0,
            };
            let to = match end.line == line {
                true => buffer.display_column(end),
                false => buffer.display_width(line) + 1,
            };
            if to <= from {
                continue;
            }
            cx.quad(Quad::filled(
                Rect::from_xywh(
                    painting.left + from as f32 * painting.cell.width,
                    top,
                    (to - from) as f32 * painting.cell.width,
                    painting.cell.height,
                ),
                color,
            ));
        }
    }

    /// Draws a line the last commit held that the file no longer does.
    fn paint_removed(
        &self,
        painting: &Painting<'_>,
        said: &str,
        top: f32,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let theme = painting.theme;
        let (bounds, cell) = (painting.bounds, painting.cell);
        cx.quad(Quad::filled(
            Rect::from_xywh(bounds.left(), top, bounds.size.width, cell.height),
            theme.colors.danger.alpha(theme.emphasis.change),
        ));
        cx.quad(Quad::filled(
            Rect::from_xywh(bounds.left(), top, CHANGE_WIDTH, cell.height),
            theme.colors.danger,
        ));
        let mark = glyphs.shape('−', painting.font, cx);
        cx.text(
            Point::new(painting.left - GUTTER_GAP - cell.width, top),
            mark,
            theme.colors.danger,
        );
        self.paint_plain(painting, said, top, theme.colors.text_muted, glyphs, cx);
    }

    /// Draws the mark standing for the lines left out between two excerpts.
    fn paint_gap(
        &self,
        painting: &Painting<'_>,
        top: f32,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let theme = painting.theme;
        cx.quad(Quad::filled(
            Rect::from_xywh(
                painting.bounds.left(),
                top + painting.cell.height / 2.0,
                painting.bounds.size.width,
                1.0,
            ),
            theme.colors.border_variant,
        ));
        self.paint_plain(painting, "⋯", top, theme.colors.text_subtle, glyphs, cx);
    }

    /// Draws `said` along the text column in `color`, a character at a time.
    fn paint_plain(
        &self,
        painting: &Painting<'_>,
        said: &str,
        top: f32,
        color: Rgba,
        glyphs: &mut Glyphs,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let mut column = 0;
        for ch in said.chars() {
            let width = match ch {
                '\t' => 4 - column % 4,
                _ => 1,
            };
            let drawn = column;
            column += width;
            if ch.is_whitespace() {
                continue;
            }
            let x = painting.left + drawn as f32 * painting.cell.width;
            if x > painting.bounds.right() {
                break;
            }
            let run = glyphs.shape(ch, painting.font, cx);
            cx.text(Point::new(x, top), run, color);
        }
    }

    /// Takes the presses on the headings that open a file, and the press and
    /// drag over the text that place the cursor and select.
    fn regions(&self, painting: &Painting<'_>, drawn: &[Row], cx: &mut PaintContext<'_, '_, M>) {
        let (bounds, cell) = (painting.bounds, painting.cell);
        if let Some(on_select) = self.on_select.clone() {
            let place = Rc::new(Drawn {
                top: bounds.top(),
                left: painting.left,
                cell,
                lines: drawn
                    .iter()
                    .map(|row| match row {
                        Row::Line(index, line) => Some((*index, *line)),
                        _ => None,
                    })
                    .collect(),
                files: painting.files.clone(),
            });
            if place.lines.iter().any(Option::is_some) {
                cx.draggable(
                    bounds,
                    PointerCursor::Text,
                    Arc::new(move |event| {
                        let (file, anchor) = place.place(event.start).unwrap_or_default();
                        let head = match place.place(event.current) {
                            Some((reached, head)) if reached == file => head,
                            _ => anchor,
                        };
                        on_select(event.phase, file, anchor, head)
                    }),
                    None,
                );
            }
        }
        if let Some(on_open) = self.on_open.clone() {
            for (at, row) in drawn.iter().enumerate() {
                if let Row::Header(index) = row {
                    let top = bounds.top() + at as f32 * cell.height;
                    cx.interactive(
                        Rect::from_xywh(bounds.left(), top, bounds.size.width, cell.height),
                        on_open(*index),
                    );
                }
            }
        }
    }
}
