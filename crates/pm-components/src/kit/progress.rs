//! The spinner that shows work going on and the turning icon, on the frame's
//! clock, which a snapshot holds still; the ring is rustle's [`rustle::dashed_spinner`].

use rustle::Modifier;

use crate::kit::frame;
use crate::kit::icon::IconName;
use crate::kit::theme::{self, Tone, sp};

/// How long a spinner takes to turn once, in milliseconds.
const TURN_MS: f32 = 1000.0;

/// Emits a spinner `steps` of the spacing scale across in `tone`: a dashed
/// ring, the loading icon, turning once a second, read from the frame's clock when painted, so
/// it turns without composing: `glyph(IconName::Loading, steps, tone)` with `animate_spin()`.
#[track_caller]
pub fn spinner(steps: f32, tone: impl Into<Tone>) {
    rustle::dashed_spinner(sp(steps), theme::color(tone), || {
        let now = frame::clock_ms();
        frame::animate();
        (now % TURN_MS) / TURN_MS * 360.0
    });
}

/// Emits `name` turning once a second, `steps` of the spacing scale across,
/// in `tone`: `glyph(name, steps, tone)` with `animate_spin()` for an icon
/// other than the loading ring. It turns by composing again each frame, so
/// prefer [`spinner`] for the loading ring.
#[track_caller]
pub fn spinning_icon(name: IconName, steps: f32, tone: impl Into<Tone>) {
    let turned = (frame::clock_ms() % TURN_MS) / TURN_MS * 360.0;
    frame::animate();
    frame::request_compose();
    crate::kit::icon::icon_with(Modifier::new().rotate(turned), name, sp(steps), tone);
}
