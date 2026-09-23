//! The whole file in miniature, down the right edge of a pane of text.
//!
//! A line is drawn as the shape its text makes rather than as its text: a
//! bar a pixel or so wide for every character, in the colour the character
//! is highlighted in, three pixels to a line. What that shows is the outline
//! of the file — where the long functions are, where the comments are, how
//! far down the view is — which is what a reader looks down the edge for.
//!
//! A file taller than the strip scrolls through it in step with the view, so
//! the top of the file is at the top of the strip when the view is at the
//! top, and the end at the end.

use pm_core::{Change, ChangeKind};
use pm_gfx::{Quad, Rect, Rgba};
use pm_text::Buffer;
use pm_ui::{PaintContext, Theme};

use crate::editor::view::tint;

/// How wide the strip is.
pub const MINIMAP_WIDTH: f32 = 110.0;

/// How tall one line of the file is drawn in the strip.
const ROW: f32 = 3.0;

/// How much of that height the line's own marks take.
const INK: f32 = 2.0;

/// How wide one character is drawn in the strip.
const COLUMN: f32 = 1.0;

/// How far the marks sit from the strip's left edge.
const INSET: f32 = 6.0;

/// How strongly a line's marks are drawn, against the colour they are in.
const INK_ALPHA: f32 = 0.55;

/// How wide the mark of a changed line is, at the strip's left edge.
const CHANGE_WIDTH: f32 = 2.0;

/// Where the strip is and which part of the file it shows.
#[derive(Clone, Copy, Debug)]
pub struct Minimap {
    /// The strip itself.
    pub area: Rect,
    /// The first line of the file the strip shows.
    pub first: usize,
    /// How many lines the strip has room for.
    pub capacity: usize,
    /// How many lines the file has.
    pub total: usize,
}

impl Minimap {
    /// The strip at the right edge of `bounds`, for a file of `total` lines
    /// being viewed from `scroll` with `rows` lines of room.
    pub fn of(bounds: Rect, total: usize, scroll: usize, rows: usize) -> Self {
        let area = Rect::from_xywh(
            bounds.right() - MINIMAP_WIDTH,
            bounds.top(),
            MINIMAP_WIDTH,
            bounds.size.height,
        );
        let capacity = (area.size.height / ROW).floor().max(1.0) as usize;
        let first = match total > capacity {
            true => {
                let travel = total.saturating_sub(rows).max(1) as f32;
                let along = (scroll as f32 / travel).min(1.0);
                ((total - capacity) as f32 * along).round() as usize
            }
            false => 0,
        };

        Self {
            area,
            first,
            capacity,
            total,
        }
    }

    /// The line of the file drawn at height `y`.
    pub fn line_at(&self, y: f32) -> usize {
        let row = ((y - self.area.top()) / ROW).floor().max(0.0) as usize;
        (self.first + row).min(self.total.saturating_sub(1))
    }

    /// The lines of the file the strip shows.
    fn lines(&self) -> std::ops::Range<usize> {
        self.first..(self.first + self.capacity).min(self.total)
    }

    /// The top of `line` in the strip.
    fn top_of(&self, line: usize) -> f32 {
        self.area.top() + line.saturating_sub(self.first) as f32 * ROW
    }

    /// Draws the strip: every line it has room for, the changes against the
    /// index, and the band showing the lines the view has on screen.
    pub fn paint<M>(
        &self,
        buffer: &mut Buffer,
        changes: &[Change],
        view: std::ops::Range<usize>,
        lit: bool,
        theme: &Theme,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        cx.quad(Quad::filled(self.area, theme.colors.background));
        cx.quad(Quad::filled(
            Rect::from_xywh(
                self.area.left(),
                self.area.top(),
                1.0,
                self.area.size.height,
            ),
            theme.colors.border_variant,
        ));
        cx.push_clip(self.area);

        let lines = self.lines();
        let highlights = buffer.highlights(lines.clone());
        let tab = buffer.tab_width().max(1);
        for line in lines {
            let top = self.top_of(line);
            let mut column = 0;
            let mut run: Option<(usize, usize, Rgba)> = None;
            for (index, ch) in buffer.line_chars(line).enumerate() {
                let width = match ch {
                    '\t' => tab - column % tab,
                    _ => 1,
                };
                let color = highlights
                    .at(line, index)
                    .map_or(theme.colors.text, |highlight| tint(highlight, theme));
                run = match (run, ch.is_whitespace()) {
                    (Some(open), true) => {
                        self.ink(open, top, cx);
                        None
                    }
                    (Some((start, end, open)), false) if open == color && end == column => {
                        Some((start, end + width, open))
                    }
                    (Some(open), false) => {
                        self.ink(open, top, cx);
                        Some((column, column + width, color))
                    }
                    (None, true) => None,
                    (None, false) => Some((column, column + width, color)),
                };
                column += width;
            }
            if let Some(open) = run {
                self.ink(open, top, cx);
            }
        }

        self.paint_changes(changes, theme, cx);
        self.paint_view(view, lit, theme, cx);
        cx.pop_clip();
    }

    /// Draws one run of characters of one colour on the line at `top`.
    fn ink<M>(
        &self,
        (start, end, color): (usize, usize, Rgba),
        top: f32,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        cx.quad(Quad::filled(
            Rect::from_xywh(
                self.area.left() + INSET + start as f32 * COLUMN,
                top,
                (end - start) as f32 * COLUMN,
                INK,
            ),
            color.alpha(INK_ALPHA),
        ));
    }

    /// Marks the lines that differ from the index at the strip's left edge.
    fn paint_changes<M>(
        &self,
        changes: &[Change],
        theme: &Theme,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        for change in changes {
            let color = match change.kind {
                ChangeKind::Added => theme.colors.success,
                ChangeKind::Modified => theme.colors.warning,
                ChangeKind::Removed => theme.colors.danger,
            };
            let lines = match change.lines.is_empty() {
                true => change.anchor()..change.anchor() + 1,
                false => change.lines.clone(),
            };
            let (first, last) = (lines.start.max(self.first), lines.end.min(self.lines().end));
            if first >= last {
                continue;
            }
            cx.quad(Quad::filled(
                Rect::from_xywh(
                    self.area.left() + 1.0,
                    self.top_of(first),
                    CHANGE_WIDTH,
                    (last - first) as f32 * ROW,
                ),
                color,
            ));
        }
    }

    /// Draws the band over the lines the view has on screen.
    fn paint_view<M>(
        &self,
        view: std::ops::Range<usize>,
        lit: bool,
        theme: &Theme,
        cx: &mut PaintContext<'_, '_, M>,
    ) {
        let top = self.top_of(view.start.max(self.first));
        let height = view.len().max(1) as f32 * ROW;
        let strength = match lit {
            true => theme.emphasis.scrollbar_active,
            false => theme.emphasis.scrollbar,
        };
        cx.quad(Quad::filled(
            Rect::from_xywh(self.area.left(), top, self.area.size.width, height),
            theme.colors.text_subtle.alpha(strength * 0.5),
        ));
    }
}
