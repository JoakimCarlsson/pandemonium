//! Small pieces the screens share: a line of text, a glyph, a check box, the
//! note a list shows when it has nothing to show and the bar that tops a panel.

use crate::theme;
use rustle::Color;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, TextOverflow};

/// Emits a line of `content` at `pixels` tall type with a line height of `ratio` times that,
/// in `tone`, ended with an ellipsis when the box it sits in is too narrow for it.
#[track_caller]
pub fn line(content: impl Into<String>, pixels: u32, ratio: f32, tone: impl Into<Color>) {
    rustle::basic_text(
        Modifier::new(),
        content,
        theme::text_style(pixels, ratio)
            .color(theme::color(tone))
            .max_lines(1),
        TextOverflow::Ellipsis,
    );
}

/// Emits a line of `content` at medium weight in the text role, `pixels` tall with a line
/// height of `ratio` times that: the title of a page or a dialog.
#[track_caller]
pub fn title(content: impl Into<String>, pixels: u32, ratio: f32) {
    rustle::basic_text(
        Modifier::new(),
        content,
        theme::text_style(pixels, ratio)
            .font_weight(rustle::FontWeight::MEDIUM)
            .color(theme::color(theme::Role::Text))
            .max_lines(1),
        TextOverflow::Ellipsis,
    );
}

/// Emits an icon drawn in a tone in a square of a number of steps on a side.
pub use crate::icons::icon as glyph;

/// Emits a box that takes the room left in its row and cuts a line of `content` short at its edge.
#[track_caller]
pub fn clipped(content: impl Into<String>, pixels: u32, tone: impl Into<Color>) {
    rustle::basic_text(
        Modifier::new().weight(1.0),
        content,
        theme::line(pixels, tone),
        TextOverflow::Ellipsis,
    );
}

/// Emits a box that takes the room left in its row and holds nothing.
#[track_caller]
pub fn spacer() {
    rustle::spacer(rustle::Modifier::new().weight(1.0));
}

/// Emits `content` wrapped to `width`, set in `pixels` tall type with a line height of `ratio` times that, in `tone`, with a gap where it has a blank line.
///
/// Each run of lines between blank lines is one text that the text layout breaks at spaces, so a word wider than `width` is cut where it must be, and text selected across a line the wrapping broke is copied as one line.
#[track_caller]
pub fn paragraph(content: &str, pixels: u32, ratio: f32, tone: impl Into<Color>, width: f32) {
    let kind = theme::text_style(pixels, ratio).color(theme::color(tone.into()));
    wrapped(content, kind, width.max(32.0), Some(theme::sp(3)));
}

/// Emits `content` set in `kind`, broken at spaces by the text layout into lines of at most
/// `width` logical pixels, whatever room its parent has: Compose's `Text` in a box of that
/// width. With a `blank` gap, each
/// run of lines between blank lines is a text of its own and each blank line a gap that
/// tall; without one, the whole of `content` is one text.
#[track_caller]
pub fn wrapped(content: &str, kind: rustle::TextStyle, width: f32, blank: Option<f32>) {
    let kind = kind.wrap(rustle::TextWrap::Word);
    let at = || {
        Modifier::new()
            .wrap_content_width(rustle::BoxAlignment::TopStart, true)
            .width_in(0.0, width)
    };
    let Some(gap) = blank else {
        rustle::text_styled(at(), content, kind);
        return;
    };
    rustle::column_arranged(
        Modifier::new(),
        Arrangement::Start,
        Alignment::Start,
        || {
            for run in content
                .split('\n')
                .collect::<Vec<_>>()
                .split(|line| line.is_empty())
            {
                if run.is_empty() {
                    rustle::spacer(Modifier::new().height(gap));
                } else {
                    rustle::text_styled(at(), run.join("\n"), kind);
                }
            }
        },
    );
}

/// Emits `content` set in `pixels` tall type with a line height of `ratio` times that, in
/// `tone`, with `modifier`, broken at spaces by the text layout into as many lines as the
/// width the modifier and the parent leave it needs: Compose's `Text` with no line limit.
#[track_caller]
pub fn wrapping(
    modifier: Modifier,
    content: impl Into<String>,
    pixels: u32,
    ratio: f32,
    tone: impl Into<Color>,
) {
    rustle::basic_text(
        modifier,
        content,
        theme::text_style(pixels, ratio)
            .color(theme::color(tone))
            .wrap(rustle::TextWrap::Word),
        TextOverflow::Clip,
    );
}

/// Emits one line of `words` `pixels` tall in `tone`, inset `x` theme steps
/// at either end and `y` above and below: what a list, menu or view says
/// when it has nothing to show, or is still busy.
#[track_caller]
pub fn note(
    x: impl Into<f64>,
    y: impl Into<f64>,
    words: impl Into<String>,
    pixels: u32,
    tone: impl Into<Color>,
) {
    rustle::column_arranged(
        Modifier::new().padding_xy(theme::sp(x), theme::sp(y)),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::basic_text(
                Modifier::new(),
                words,
                theme::line(pixels, tone),
                TextOverflow::Ellipsis,
            )
        },
    );
}

/// Emits a bar across its parent `height` theme steps tall, ruled along its
/// bottom and inset `inset` theme steps at either end, holding `content` in a
/// row `gap` theme steps apart: the header that tops panels, lists and steps.
#[track_caller]
pub fn bar(
    height: impl Into<f64>,
    inset: impl Into<f64>,
    gap: impl Into<f64>,
    content: impl FnOnce(),
) {
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(height))
            .border_bottom(1.0, theme::color(theme::Role::Border))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(inset), 0.0),
        Arrangement::SpacedBy(theme::sp(gap)),
        Alignment::Center,
        content,
    );
}
