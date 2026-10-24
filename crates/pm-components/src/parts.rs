//! Small pieces the screens share: a line of text, a glyph, a check box.

use pm_icon::IconName;
use pm_style::{
    Color, flex_1, leading_ratio, min_w_px, overflow_hidden, size, text_color, text_px, truncate,
};
use pm_ui::{Node, node, text};

/// How wide, as a share of its height, a character of body text is taken to be.
const CHARACTER_WIDTH: f32 = 0.53;

/// A line of `content` at `pixels` tall type with a line height of `ratio` times that, in `tone`,
/// ended with an ellipsis when the box it sits in is too narrow for it.
pub fn line(content: impl Into<String>, pixels: u32, ratio: f32, tone: impl Into<Color>) -> Node {
    text(content).styles([
        text_px(pixels),
        leading_ratio(ratio),
        text_color(tone),
        truncate(),
        min_w_px(0),
    ])
}

/// `name` drawn in `tone` in a square `steps` on a side.
pub fn glyph(name: IconName, steps: f32, tone: impl Into<Color>) -> Node {
    node().styles([size(steps), text_color(tone)]).icon(name)
}

/// A box that takes the room left in its row and cuts a line of `content` short at its edge.
pub fn clipped(content: impl Into<String>, pixels: u32, tone: impl Into<Color>) -> Node {
    node()
        .styles([flex_1(), min_w_px(0), overflow_hidden()])
        .child(line(content, pixels, 1.4, tone).styles([truncate()]))
}

/// A box that takes the room left in its row and holds nothing.
pub fn spacer() -> Node {
    node().styles([flex_1()])
}

/// The lines `content` breaks into when each may hold as many characters of
/// `pixels` tall type as fit in `width` logical pixels, since text does not wrap.
pub fn wrap(content: &str, pixels: u32, width: f32) -> Vec<String> {
    let character = pixels as f32 * pm_style::text_scale() * CHARACTER_WIDTH;
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
