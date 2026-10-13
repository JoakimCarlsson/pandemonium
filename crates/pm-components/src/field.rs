//! The text input: a box holding one editable line.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, h, items_center, px, ring, ring_color, rounded,
    text_base, text_color, w_full,
};
use pm_text::Field;
use pm_ui::{Node, node, text};

/// A field showing `field`, or `placeholder` while it is empty, with a ring
/// and a blinking caret while it holds keyboard focus under the name `id`.
pub fn text_input(field: &Field, placeholder: &str, id: FocusId) -> Node {
    let (content, tone, caret) = if field.text().is_empty() {
        (placeholder, Role::Subtle, 0)
    } else {
        (field.text(), Role::Text, field.caret())
    };
    let line = text(content)
        .styles([text_base(), text_color(tone)])
        .caret(caret, id);
    node()
        .styles([
            flex(),
            items_center(),
            w_full(),
            h(10),
            px(3),
            rounded(Radius::Lg),
            bg(Role::Background),
            border(),
            border_color(Role::Border),
            border_color(Role::Focused).focus(),
            ring(2).focus(),
            ring_color(Role::Focused).focus(),
        ])
        .focusable(id)
        .child(line)
}
