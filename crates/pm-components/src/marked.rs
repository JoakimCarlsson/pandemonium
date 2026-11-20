//! Marked lines of the app's text views: syntax colours, fills behind matches,
//! underlines for problems and links, selections, carets, indent guides and
//! drop points, named by the theme's roles, resolved to colours and drawn by
//! [`rustle::marked_text`] when a line is emitted. A line of the code text
//! tells its [`TextLines`] where it was drawn, and a line inside a
//! [`rustle::selection_container`] takes part in its selection.

use crate::target::Target;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use rustle::{Color, MarkPaint, MarkedTextOptions, Modifier, TextLayout, TextMark};

use crate::text_lines::TextLines;
use crate::theme::{self, Theme};

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
    pub tone: Color,
    /// How.
    pub paint: Paint,
}

impl Mark {
    /// Paints the bytes `range` in `tone` the way `paint` says.
    pub fn new(range: Range<usize>, tone: impl Into<Color>, paint: Paint) -> Self {
        Self {
            range,
            tone: tone.into(),
            paint,
        }
    }
}

/// Whether a blinking caret is lit: on for [`CARET_BLINK_MS`] and off for as long, by the
/// time of the frame clock, which an effect follows frame by frame and waits on until the
/// next change, as a text field's cursor blinks on `withFrameMillis`.
pub fn caret_lit(look: CaretLook) -> bool {
    let lit = rustle::remember(|| true);
    let driven = lit.clone();
    let steady = look.steady;
    rustle::launched_effect(steady, move |scope| async move {
        if steady {
            return;
        }
        loop {
            let now = scope.next_frame().await.as_secs_f32() * 1000.0;
            let phase = (now / CARET_BLINK_MS).floor();
            let on = (phase as u64).is_multiple_of(2);
            if driven.get() != on {
                driven.set(on);
            }
            let until = ((phase + 1.0) * CARET_BLINK_MS - now).max(1.0);
            scope
                .delay(std::time::Duration::from_secs_f32(until / 1000.0))
                .await;
        }
    });
    steady || lit.get()
}

/// The colour selected text is highlighted in, in `theme`.
pub fn selection_color(theme: &Theme) -> rustle::Color {
    theme::selection_colors(theme).background_color
}

/// Emits one line of `content` set in `kind` with `marks` painted over and
/// under it, its carets drawn as `look` says: `text(content).styles([...]).marks(marks)`.
///
/// Inked stretches become the spans of one rich text; fills, selections and
/// guides are drawn behind it, underlines, drop points and carets over it,
/// from the layout the text reports when it is laid out. Carets blink with the
/// frame clock unless `look` keeps them steady.
#[track_caller]
pub fn marked_line(
    modifier: Modifier,
    content: &str,
    kind: rustle::TextStyle,
    marks: Vec<Mark>,
    look: CaretLook,
) {
    marked(modifier, content, kind, marks, look, None);
}

/// Emits one line of a text drawn line by line, as a [`marked_line`] that is
/// also the piece of the field `owner` starting at byte `start` of its text,
/// whose layout and place it tells `lines` once it is placed, with the caret,
/// selection and composition given as [`Paint::Caret`], [`Paint::Selected`]
/// and [`Paint::Underline`] marks (add them only while the app keeps focus on `owner`).
/// `lines` then answers the app's questions about the text as drawn.
pub fn marked_field_line(
    modifier: Modifier,
    lines: &TextLines,
    (owner, start): (Target, usize),
    content: &str,
    kind: rustle::TextStyle,
    marks: Vec<Mark>,
    look: CaretLook,
) {
    marked(
        modifier,
        content,
        kind,
        marks,
        look,
        Some((lines, owner, start)),
    );
}

/// The rustle mark `mark` stands for, its tone resolved in `theme`; a
/// selection is a fill in the selection colour.
fn resolved(mark: &Mark, theme: &Theme) -> TextMark {
    let tone = match mark.paint {
        Paint::Selected => selection_color(theme),
        _ => mark.tone,
    };
    let paint = match mark.paint {
        Paint::Ink => MarkPaint::Ink,
        Paint::Fill | Paint::Selected => MarkPaint::Fill,
        Paint::Wave => MarkPaint::Wave,
        Paint::Underline => MarkPaint::Underline,
        Paint::Caret => MarkPaint::Caret,
        Paint::Guide => MarkPaint::Guide,
        Paint::Drop => MarkPaint::Drop,
    };
    TextMark::new(mark.range.clone(), tone, paint)
}

/// Emits a marked line, told to the lines holder as the piece of a field
/// starting at a byte of its text when `piece` names them; inside a
/// [`rustle::selection_container`] it takes part in the selection.
#[track_caller]
fn marked(
    modifier: Modifier,
    content: &str,
    kind: rustle::TextStyle,
    marks: Vec<Mark>,
    look: CaretLook,
    piece: Option<(&TextLines, Target, usize)>,
) {
    let theme = theme::theme();
    let caret_visible = marks.iter().any(|mark| mark.paint == Paint::Caret) && caret_lit(look);
    let marks: Vec<TextMark> = marks.iter().map(|mark| resolved(mark, &theme)).collect();
    let shaped: Rc<RefCell<Option<Rc<TextLayout>>>> = Rc::default();
    let (stored, placed) = (shaped.clone(), shaped);
    let modifier = match piece {
        Some((lines, owner, start)) => lines.line(modifier, owner, start, placed),
        None => modifier,
    };
    rustle::marked_text(
        modifier,
        content,
        kind,
        marks,
        MarkedTextOptions {
            caret_shape: look.shape,
            caret_visible,
            line_height: Some(kind.line_height),
            on_layout: Some(Rc::new(move |layout| *stored.borrow_mut() = Some(layout))),
        },
    );
}
