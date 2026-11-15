//! A glyph turning round while something is busy, such as a push under way.

use std::time::Duration;

use rustle::prelude::*;

/// How long one turn of a spinning glyph takes.
const TURN: Duration = Duration::from_millis(900);

/// Emits `glyph`, `side` logical pixels square and painted `tint`, turning
/// round for as long as it stays in the composition.
#[track_caller]
pub fn spinning_icon(glyph: Icon, side: f32, tint: Color) {
    let angle = infinite_float_repeating(0.0, 360.0, TURN, Easing::Linear);
    stack_aligned(
        Modifier::new().size(side, side).rotate(angle.get()),
        BoxAlignment::Center,
        move || icon(glyph, side, tint),
    );
}

/// Emits a ring with a gap, `side` logical pixels square and painted `tint`,
/// turning round as the mark of something busy.
#[track_caller]
pub fn spinner(side: f32, tint: Color) {
    spinning_icon(phosphor::CIRCLE_NOTCH, side, tint);
}
