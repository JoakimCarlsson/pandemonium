//! Small pieces the screens share: a line of text, a glyph, a check box.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, bg, border, border_color, cursor_pointer, flex, flex_1, items_center,
    justify_center, leading_ratio, min_w_px, overflow_hidden, rounded, size, text_color, text_px,
    truncate,
};
use pm_ui::{Node, node, text};

/// How wide, as a share of its height, a character of body text is taken to be.
const CHARACTER_WIDTH: f32 = 0.48;

/// A line of `content` at `pixels` tall type with a line height of `ratio` times that, in `tone`.
pub(crate) fn line(
    content: impl Into<String>,
    pixels: u32,
    ratio: f32,
    tone: impl Into<Color>,
) -> Node {
    text(content).styles([text_px(pixels), leading_ratio(ratio), text_color(tone)])
}

/// `name` drawn in `tone` in a square `steps` on a side.
pub(crate) fn glyph(name: IconName, steps: f32, tone: impl Into<Color>) -> Node {
    node().styles([size(steps), text_color(tone)]).icon(name)
}

/// A box that takes the room left in its row and cuts a line of `content` short at its edge.
pub(crate) fn clipped(content: impl Into<String>, pixels: u32, tone: impl Into<Color>) -> Node {
    node()
        .styles([flex_1(), min_w_px(0), overflow_hidden()])
        .child(line(content, pixels, 1.4, tone).styles([truncate()]))
}

/// A box that takes the room left in its row and holds nothing.
pub(crate) fn spacer() -> Node {
    node().styles([flex_1()])
}

/// A check box answering to `id`, ticked while `checked`.
pub(crate) fn check_box(checked: bool, id: FocusId) -> Node {
    let mut styles = vec![
        flex(),
        items_center(),
        justify_center(),
        size(4),
        rounded(Radius::Sm),
        border(),
        cursor_pointer(),
    ];
    if checked {
        styles.extend([bg(Role::Accent), border_color(Role::Accent)]);
    } else {
        styles.extend([border_color(Role::Subtle)]);
    }
    let boxed = node().styles(styles);
    let boxed = if checked {
        boxed.child(glyph(IconName::Check, 3.0, Role::OnAccent))
    } else {
        boxed
    };
    boxed.focusable(id)
}

/// The lines `content` breaks into when each may hold as many characters of
/// `pixels` tall type as fit in `width` logical pixels, since text does not wrap.
pub(crate) fn wrap(content: &str, pixels: u32, width: f32) -> Vec<String> {
    let columns = ((width / (pixels as f32 * CHARACTER_WIDTH)) as usize).max(8);
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
