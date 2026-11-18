//! Marked lines of the app's text views: syntax colours, fills behind matches,
//! underlines for problems and links, selections, carets, indent guides and
//! drop points, named by the theme's roles, resolved to colours and drawn by
//! [`rustle::marked_text`] when a line is emitted. A line of a field or of
//! the selectable conversation is also recorded where the app looks up
//! points and selections.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use rustle::{MarkPaint, MarkedTextOptions, Modifier, TextLayout, TextMark};

use crate::kit::focus::FocusId;
use crate::kit::frame;
use crate::kit::report::{self, FieldLine};
use crate::kit::text::Type;
use crate::kit::theme::{self, Role, Theme, Tone};

pub use rustle::CaretShape;

/// How long the caret stays on, and then off, in milliseconds.
pub const CARET_BLINK_MS: f32 = 530.0;
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

/// The rustle mark `mark` stands for, its tone resolved in `theme`; a
/// selection is a fill in the selection colour.
fn resolved(mark: &Mark, theme: &Theme) -> TextMark {
    match mark.paint {
        Paint::Selected => {
            TextMark::new(mark.range.clone(), selection_color(theme), MarkPaint::Fill)
        }
        paint => TextMark::new(
            mark.range.clone(),
            theme::color_in(theme, mark.tone),
            match paint {
                Paint::Ink => MarkPaint::Ink,
                Paint::Fill | Paint::Selected => MarkPaint::Fill,
                Paint::Wave => MarkPaint::Wave,
                Paint::Underline => MarkPaint::Underline,
                Paint::Caret => MarkPaint::Caret,
                Paint::Guide => MarkPaint::Guide,
                Paint::Drop => MarkPaint::Drop,
            },
        ),
    }
}

/// Emits a marked line, recorded as the piece of a field starting at a byte
/// of its shown text when `piece` names the field and the byte, and as a
/// piece of the selectable text the line is composed in, if any.
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
    let caret_visible = marks.iter().any(|mark| mark.paint == Paint::Caret) && caret_lit(look);
    let marks: Vec<TextMark> = marks.iter().map(|mark| resolved(mark, &theme)).collect();
    let shaped: Rc<RefCell<Option<Rc<TextLayout>>>> = Rc::default();
    let (stored, placed, scoped) = (shaped.clone(), shaped.clone(), shaped);
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
    rustle::marked_text(
        modifier,
        content,
        kind.style(),
        marks,
        MarkedTextOptions {
            caret_shape: look.shape,
            caret_visible,
            line_height: Some(kind.line_height()),
            on_layout: Some(Rc::new(move |layout| *stored.borrow_mut() = Some(layout))),
        },
    );
}
