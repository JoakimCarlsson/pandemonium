//! Things that move on their own: spinner, pulse, ping and bounce.

use pm_style::{
    Color, absolute, animate_bounce, animate_ping, animate_pulse, animate_spin, bg, flex, gap,
    items_center, justify_center, left, relative, rounded_full, size, top,
};
use pm_ui::{Node, node};

/// A dot orbiting the middle of its box, turning once a second.
pub fn spinner(tone: Color) -> Node {
    node()
        .styles([relative(), size(10), animate_spin()])
        .child(node().styles([
            absolute(),
            top(0),
            left(4),
            size(2),
            rounded_full(),
            bg(tone),
        ]))
}

/// A dot that fades to half and back.
pub fn pulse_dot(tone: Color) -> Node {
    node().styles([size(4), rounded_full(), bg(tone), animate_pulse()])
}

/// A dot with a ripple that grows from it and fades.
pub fn ping_dot(tone: Color) -> Node {
    node()
        .styles([
            relative(),
            flex(),
            items_center(),
            justify_center(),
            size(4),
        ])
        .child(node().styles([
            absolute(),
            size(4),
            rounded_full(),
            bg(tone.alpha(0.7)),
            animate_ping(),
        ]))
        .child(node().styles([size(3), rounded_full(), bg(tone)]))
}

/// Three dots that bounce.
pub fn bouncing_dots(tone: Color) -> Node {
    node().styles([flex(), items_center(), gap(1.5)]).children(
        (0..3).map(|_| node().styles([size(3), rounded_full(), bg(tone), animate_bounce()])),
    )
}
