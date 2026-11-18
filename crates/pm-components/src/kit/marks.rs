//! Lines of text with stretches painted apart from the rest: syntax colours,
//! fills behind matches, wavy and straight underlines for problems and
//! links, selections, carets, indent guides and the drop point of dragged
//! text.
//!
//! This is the hot path of the code view, the terminal, the diff and the
//! minimap, so a [`marked_line`] is one rich text node: the inked stretches
//! become its spans, shaped once by the renderer, and everything else is
//! drawn over or under it from the layout the node reports, when the line is
//! painted. Build only the lines that are on screen, as before.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use rustle::{Affinity, Canvas, Modifier, TextLayout, TextSpan};

use crate::kit::focus::FocusId;
use crate::kit::frame;
use crate::kit::report::{self, FieldLine};
use crate::kit::text::Type;
use crate::kit::theme::{self, Role, Theme, Tone};

/// How long the caret stays on, and then off, in milliseconds.
pub const CARET_BLINK_MS: f32 = 530.0;

/// The width of a bar caret in logical pixels.
pub const CARET_WIDTH: f32 = 1.5;

/// How long each piece of a wavy underline is, in logical pixels.
const WAVE_STEP: f32 = 2.0;

/// How a caret is drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CaretShape {
    /// A thin upright bar before the character it is at.
    #[default]
    Bar,
    /// The character it is at, filled.
    Block,
    /// A line under the character it is at.
    Underline,
}

/// How the carets of a line are drawn: their shape, and whether they stay lit rather than blink.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaretLook {
    /// The shape.
    pub shape: CaretShape,
    /// Whether they stay lit.
    pub steady: bool,
}

/// How a stretch of a line of text is painted apart from the rest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Paint {
    /// Its letters in another colour.
    Ink,
    /// A box of colour behind it.
    Fill,
    /// A wavy line under it.
    Wave,
    /// A straight line under it.
    Underline,
    /// Shown as selected, as the selection of a field is.
    Selected,
    /// A blinking caret before its first byte, as the caret of a field is.
    Caret,
    /// A thin upright line the height of the line at the column its first
    /// number counts, in spaces from the left, which may lie past the end of the text.
    Guide,
    /// A steady upright line before its first byte, in its colour: where dragged text would be dropped.
    Drop,
}

/// A stretch of a line of text painted apart from the rest.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// The bytes of the text.
    pub range: Range<usize>,
    /// The colour.
    pub tone: Tone,
    /// How.
    pub paint: Paint,
}

impl Mark {
    /// Paints the bytes `range` in `tone` the way `paint` says.
    pub fn new(range: Range<usize>, tone: impl Into<Tone>, paint: Paint) -> Self {
        Self {
            range,
            tone: tone.into(),
            paint,
        }
    }
}

/// Whether `content` holds right-to-left letters, whose line the text node lays
/// out against its whole width, so its marks are placed from the line shaped
/// alone instead, as it is drawn.
fn right_to_left(content: &str) -> bool {
    content
        .chars()
        .any(|letter| matches!(letter, '\u{0590}'..='\u{08FF}' | '\u{FB1D}'..='\u{FEFF}'))
}

/// Whether a blinking caret is lit at the frame's clock, asking for the frame at which it changes.
pub fn caret_lit(look: CaretLook) -> bool {
    if look.steady {
        return true;
    }
    let now = frame::clock_ms();
    let phase = (now / CARET_BLINK_MS).floor();
    frame::wake_at((phase + 1.0) * CARET_BLINK_MS);
    (phase as u64).is_multiple_of(2)
}

/// Draws a caret at byte `byte` of `layout`, drawn with its top left at `x`, `y`, shaped as `look` says.
pub fn draw_caret(
    canvas: &mut Canvas,
    layout: &TextLayout,
    (x, y): (f32, f32),
    byte: usize,
    shape: CaretShape,
    color: rustle::Color,
) {
    let at = layout.caret(byte, Affinity::Downstream);
    let content = layout.text();
    let next = content
        .get(byte..)
        .and_then(|rest| rest.chars().next())
        .map(|character| byte + character.len_utf8());
    let edge = next
        .map(|next| layout.caret(next, Affinity::Upstream).x)
        .filter(|edge| (edge - at.x).abs() > 0.5);
    let (left, cell) = edge.map_or((at.x, at.height * 0.45), |edge| {
        (at.x.min(edge), (edge - at.x).abs())
    });
    match shape {
        CaretShape::Bar => canvas.rect(x + at.x, y + at.y, CARET_WIDTH, at.height, color),
        CaretShape::Block => {
            let faded = rustle::Color {
                a: (f32::from(color.a) * 0.45) as u8,
                ..color
            };
            canvas.rect(x + left, y + at.y, cell, at.height, faded);
        }
        CaretShape::Underline => {
            canvas.rect(x + left, y + at.y + at.height - 2.0, cell, 2.0, color)
        }
    }
}

/// Fills the stretches `range` of `layout`, drawn with its top left at `x`, `y`, cover, with `color`, line by line.
pub fn draw_fill(
    canvas: &mut Canvas,
    layout: &TextLayout,
    (x, y): (f32, f32),
    range: Range<usize>,
    color: rustle::Color,
) {
    canvas.selection(x, y, layout, range.start, range.end, color);
}

/// Draws a line under the stretch `range` of `layout`, `size` pixel type, straight or wavy.
pub fn draw_underline(
    canvas: &mut Canvas,
    layout: &TextLayout,
    (x, y): (f32, f32),
    range: Range<usize>,
    size: f32,
    wavy: bool,
    color: rustle::Color,
) {
    for rect in layout.selection_rects(range.start, range.end) {
        let line = layout.line_of_offset(range.start, Affinity::Downstream);
        let baseline = layout
            .lines()
            .get(line)
            .map_or(rect.size.height, |held| held.baseline - held.top);
        let left = x + rect.origin.x;
        let right = left + rect.size.width;
        let top = y + rect.origin.y + baseline;
        if !wavy {
            let thickness = (size / 14.0).max(1.0);
            canvas.rect(left, top + size * 0.12, right - left, thickness, color);
            continue;
        }
        let right = right.max(left + WAVE_STEP * 2.0);
        let mut from = left;
        let mut step = 0;
        while from < right {
            let lifted = if step % 2 == 0 { 0.0 } else { 1.0 };
            canvas.rect(
                from,
                top + 2.0 + lifted,
                WAVE_STEP.min(right - from),
                1.0,
                color,
            );
            from += WAVE_STEP;
            step += 1;
        }
    }
}

/// The colour selected text is highlighted in, in `theme`.
pub fn selection_color(theme: &Theme) -> rustle::Color {
    theme::color_in(theme, Tone::from(Role::Focused).alpha(0.45))
}

/// Emits one line of `content` set in `kind` with `marks` painted over and
/// under it, its carets drawn as `look` says: `text(content).styles([...]).marks(marks)`.
///
/// Inked stretches become the spans of one rich text; fills, selections and
/// guides are drawn behind it, underlines, drop points and carets over it,
/// from the layout the text reports when it is laid out. Carets blink with the
/// frame's clock unless `look` keeps them steady.
#[track_caller]
pub fn marked_line(
    modifier: Modifier,
    content: &str,
    kind: Type,
    marks: Vec<Mark>,
    look: CaretLook,
) {
    marked(modifier, content, kind, marks, look, None);
}

/// Emits one line of a text field drawn line by line, as a [`marked_line`]
/// that is also the piece of the field `owner` starting at byte `start` of
/// its shown text: `text(..).field_line(owner, start).marks(..)`, with
/// `.caret(..)`, `.selection(..)` and `.marked(..)` given as [`Paint::Caret`],
/// [`Paint::Selected`] and [`Paint::Underline`] marks (add them only while
/// `frame::is_focused(owner)`). The frame's report then answers the host's
/// questions about the field (`offset_in`, `vertical`, `line_edge`,
/// `visual_step`, `caret_rect`) from the lines drawn, as the old output did.
#[track_caller]
pub fn marked_field_line(
    modifier: Modifier,
    owner: FocusId,
    start: usize,
    content: &str,
    kind: Type,
    marks: Vec<Mark>,
    look: CaretLook,
) {
    marked(modifier, content, kind, marks, look, Some((owner, start)));
}

/// Emits a marked line, recorded as the piece of a field starting at a byte
/// of its shown text when `piece` names the field and the byte.
#[track_caller]
fn marked(
    modifier: Modifier,
    content: &str,
    kind: Type,
    marks: Vec<Mark>,
    look: CaretLook,
    piece: Option<(FocusId, usize)>,
) {
    let theme = theme::theme();
    let style = kind.style();
    let mut inked: Vec<&Mark> = marks
        .iter()
        .filter(|mark| mark.paint == Paint::Ink)
        .collect();
    inked.sort_by_key(|mark| mark.range.start);
    let mut spans = Vec::new();
    let mut last = 0;
    for mark in &inked {
        let start = mark.range.start.clamp(last, content.len());
        let end = mark.range.end.clamp(start, content.len());
        if start > last {
            spans.push(TextSpan::new(&content[last..start]));
        }
        if end > start {
            spans.push(
                TextSpan::new(&content[start..end]).color(theme::color_in(&theme, mark.tone)),
            );
        }
        last = end;
    }
    if last < content.len() || spans.is_empty() {
        spans.push(TextSpan::new(&content[last..]));
    }
    let space = rustle::layout_text(" ", &style, None).size().width;
    let size = style.font_size;
    let height = kind.line_height();
    let unbroken =
        right_to_left(content).then(|| Rc::new(rustle::layout_spans(&spans, &style, None)));
    let shaped: Rc<RefCell<Option<Rc<TextLayout>>>> = Rc::new(RefCell::new(unbroken.clone()));
    let (stored, behind, over) = (shaped.clone(), shaped.clone(), shaped);
    let marks = Rc::new(marks);
    let (under, above) = (marks.clone(), marks);
    let theme_behind = theme.clone();
    let placed = stored.clone();
    let scoped = stored.clone();
    let modifier = match piece {
        Some((owner, start)) => modifier.on_placed(move |rect, _| {
            let Some(layout) = placed.borrow().clone() else {
                return;
            };
            report::record(|report| {
                report.field_lines.push(FieldLine {
                    id: owner,
                    start,
                    layout,
                    rect,
                })
            });
        }),
        None => modifier,
    };
    let modifier = match crate::kit::select::scope() {
        Some(id) => crate::kit::select::piece(modifier, id, Rc::from(content), scoped),
        None => modifier,
    };
    let modifier = modifier
        .on_text_layout(move |layout| {
            if unbroken.is_none() {
                *stored.borrow_mut() = Some(layout);
            }
        })
        .draw_behind(move |canvas| {
            let Some(layout) = behind.borrow().clone() else {
                return;
            };
            for mark in under.iter() {
                let color = theme::color_in(&theme_behind, mark.tone);
                match mark.paint {
                    Paint::Fill => {
                        draw_fill(canvas, &layout, (0.0, 0.0), mark.range.clone(), color)
                    }
                    Paint::Selected => draw_fill(
                        canvas,
                        &layout,
                        (0.0, 0.0),
                        mark.range.clone(),
                        selection_color(&theme_behind),
                    ),
                    Paint::Guide => {
                        canvas.rect(space * mark.range.start as f32, 0.0, 1.0, height, color)
                    }
                    _ => {}
                }
            }
        })
        .draw_with_content(move |canvas| {
            canvas.draw_content();
            let Some(layout) = over.borrow().clone() else {
                return;
            };
            let mut lit = None;
            for mark in above.iter() {
                let color = theme::color_in(&theme, mark.tone);
                let range = mark.range.clone();
                match mark.paint {
                    Paint::Underline => {
                        draw_underline(canvas, &layout, (0.0, 0.0), range, size, false, color)
                    }
                    Paint::Wave => {
                        draw_underline(canvas, &layout, (0.0, 0.0), range, size, true, color)
                    }
                    Paint::Drop => {
                        let at = layout.caret(range.start, Affinity::Downstream);
                        canvas.rect(at.x, at.y, CARET_WIDTH, at.height, color);
                    }
                    Paint::Caret if *lit.get_or_insert_with(|| caret_lit(look)) => {
                        draw_caret(canvas, &layout, (0.0, 0.0), range.start, look.shape, color)
                    }
                    _ => {}
                }
            }
        });
    rustle::LOCAL_TEXT_STYLE.provide(style, || {
        rustle::rich_text_with(modifier.height(height), spans)
    });
}
