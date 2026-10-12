//! The small controls: button, badge, switch and the avatar stack.

use pm_input::FocusId;
use pm_style::{
    Color, Radius, bg, border_2, border_color, flex, font_medium, gap, h, items_center,
    justify_center, justify_end, ml, px, py, ring, ring_color, rounded, rounded_full, shadow_md,
    size, text_color, text_sm, w,
};
use pm_ui::{Node, node, text};

/// A button labelled `label` in `tone`: it lightens under the pointer,
/// darkens while pressed and shows a ring when it has keyboard focus.
pub fn button(label: &str, tone: [Color; 3], id: FocusId) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            justify_center(),
            px(4),
            h(10),
            rounded(Radius::Lg),
            shadow_md(),
            bg(tone[1]),
            bg(tone[0]).hover(),
            bg(tone[2]).active(),
            ring(2).focus(),
            ring_color(tone[0].alpha(0.6)).focus(),
        ])
        .focusable(id)
        .child(text(label).styles([text_sm(), font_medium(), text_color(Color::White)]))
}

/// A pill in `tone` labelled `label`, the way a status is shown.
pub fn badge(label: &str, tone: Color) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            gap(1.5),
            px(3),
            py(1),
            rounded_full(),
            bg(tone.alpha(0.18)),
        ])
        .child(node().styles([size(1.5), rounded_full(), bg(tone)]))
        .child(text(label).styles([pm_style::text_xs(), font_medium(), text_color(tone)]))
}

/// A switch whose knob sits `travel` of the way along its track, from 0.0
/// (off) to 1.0 (on).
pub fn switch(travel: f32, id: FocusId) -> Node {
    let track = Color::Zinc700.mix(Color::Emerald500, travel);
    node()
        .styles([
            flex(),
            items_center(),
            w(11),
            h(6),
            px(0.5),
            rounded_full(),
            bg(track),
            bg(track.mix(Color::White, 0.12)).hover(),
            ring(2).focus(),
            ring_color(Color::Emerald400.alpha(0.6)).focus(),
        ])
        .focusable(id)
        .child(node().styles([
            size(5),
            ml(travel * 5.0),
            rounded_full(),
            bg(Color::White),
            shadow_md(),
        ]))
}

/// Overlapping circles, each tinted from `tones`, the way a group of people
/// is shown.
pub fn avatar_stack(tones: &[Color]) -> Node {
    node()
        .styles([flex(), items_center(), justify_end()])
        .children(tones.iter().enumerate().map(|(index, tone)| {
            node().styles([
                size(9),
                rounded_full(),
                bg(*tone),
                border_2(),
                border_color(Color::Zinc900),
                ml(if index == 0 { 0.0 } else { -3.0 }),
            ])
        }))
}
