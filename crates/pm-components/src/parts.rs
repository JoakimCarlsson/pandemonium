//! Small pieces the screens share: a line of text, a glyph, a check box.

use crate::kit;
use crate::theme;
use rustle::Color;
use rustle::Modifier;

/// How wide, as a share of its height, a character of body text is taken to be.
const CHARACTER_WIDTH: f32 = 0.53;

/// Emits a line of `content` at `pixels` tall type with a line height of `ratio` times that,
/// in `tone`, ended with an ellipsis when the box it sits in is too narrow for it.
#[track_caller]
pub fn line(content: impl Into<String>, pixels: u32, ratio: f32, tone: impl Into<Color>) {
    kit::text(
        Modifier::new(),
        content,
        theme::text_style(pixels, ratio)
            .color(theme::color(tone))
            .max_lines(1),
    );
}

/// Emits a line of `content` at medium weight in the text role, `pixels` tall with a line
/// height of `ratio` times that: the title of a page or a dialog.
#[track_caller]
pub fn title(content: impl Into<String>, pixels: u32, ratio: f32) {
    kit::text(
        Modifier::new(),
        content,
        theme::text_style(pixels, ratio)
            .font_weight(rustle::FontWeight::MEDIUM)
            .color(theme::color(theme::Role::Text))
            .max_lines(1),
    );
}

/// Emits an icon drawn in a tone in a square of a number of steps on a side.
pub use crate::icons::icon as glyph;

/// Emits a box that takes the room left in its row and cuts a line of `content` short at its edge.
#[track_caller]
pub fn clipped(content: impl Into<String>, pixels: u32, tone: impl Into<Color>) {
    kit::text(
        Modifier::new().weight(1.0),
        content,
        kit::line(pixels, tone),
    );
}

/// Emits a box that takes the room left in its row and holds nothing.
#[track_caller]
pub fn spacer() {
    rustle::spacer(rustle::Modifier::new().weight(1.0));
}

/// Emits the lines of `content` wrapped to `width`, one text line each, set in `pixels` tall type with a line height of `ratio` times that, in `tone`, with a gap where it has a blank line.
///
/// Lines break as the text is measured, so a word wider than `width` is cut where it must be. A line the wrapping broke at a space keeps it, so that text selected across it is copied as one line.
#[track_caller]
pub fn paragraph(content: &str, pixels: u32, ratio: f32, tone: impl Into<Color>, width: f32) {
    let kind = theme::text_style(pixels, ratio);
    let tone = tone.into();
    let lines = rustle::wrap_at_spaces(content, &kind, width.max(32.0));
    kit::column(Modifier::new(), 0, || {
        for part in lines {
            if part.end == part.start {
                rustle::spacer(Modifier::new().height(theme::sp(3)));
            } else {
                kit::text(
                    Modifier::new(),
                    &content[part.start..part.end],
                    kind.color(theme::color(tone)),
                );
            }
        }
    });
}

/// The lines `content` breaks into when each may hold as many characters of
/// `pixels` tall type as fit in `width` logical pixels, since text does not wrap.
pub fn wrap(content: &str, pixels: u32, width: f32) -> Vec<String> {
    let character = pixels as f32 * crate::theme::text_scale() * CHARACTER_WIDTH;
    let columns = ((width / character) as usize).max(8);
    let mut lines: Vec<String> = Vec::new();
    for word in content.split_whitespace() {
        match lines.last_mut() {
            Some(last) if last.chars().count() + 1 + word.chars().count() <= columns => {
                last.push(' ');
                last.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}
