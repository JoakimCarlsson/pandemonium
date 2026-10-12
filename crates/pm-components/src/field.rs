//! The text input: a box holding one editable line.

use pm_input::FocusId;
use pm_style::{
    Color, Radius, bg, border, border_color, flex, h, items_center, px, ring, ring_color, rounded,
    text_base, text_color, w_full,
};
use pm_text::Field;
use pm_ui::{Node, node, text};

/// A field showing `field`, or `placeholder` while it is empty, with a ring
/// and a blinking caret while it holds keyboard focus under the name `id`.
pub fn text_input(field: &Field, placeholder: &str, id: FocusId) -> Node {
    let (content, tone, caret) = if field.text().is_empty() {
        (placeholder, Color::Zinc500, 0)
    } else {
        (field.text(), Color::Zinc100, field.caret())
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
            bg(Color::Zinc800),
            border(),
            border_color(Color::Zinc700),
            border_color(Color::Blue500).focus(),
            ring(2).focus(),
            ring_color(Color::Blue500.alpha(0.35)).focus(),
        ])
        .focusable(id)
        .child(line)
}
