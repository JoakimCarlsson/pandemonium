//! The progress bar: how far along something is.

use pm_style::{Color, Role, bg, flex, h, overflow_hidden, rounded_full, w_frac, w_full};
use pm_ui::{Node, node};

/// A track with a fill covering `fraction` of it, from 0.0 to 1.0.
pub fn progress_bar(fraction: f32, tone: Color) -> Node {
    let percent = (fraction.clamp(0.0, 1.0) * 100.0).round();
    node()
        .styles([
            flex(),
            w_full(),
            h(2),
            rounded_full(),
            overflow_hidden(),
            bg(Role::SurfaceHover),
        ])
        .child(node().styles([w_frac(percent, 100), h(2), rounded_full(), bg(tone)]))
}
