//! Progress drawn as rings, and the spinner that shows work going on.

use rustle::Modifier;

use crate::kit::frame;
use crate::kit::icon::IconName;
use crate::kit::theme::{self, Tone, sp};

/// How long a spinner takes to turn once, in milliseconds.
const TURN_MS: f32 = 1000.0;

/// Emits concentric rings in the largest square `modifier` gives, the first
/// outermost, each `width` pixels thick and `gap` apart: each is `share` of a
/// full turn, from nought to one, filled clockwise from the top in its tone,
/// and the rest of it in `track`. Rings that no longer fit are left out:
/// `.rings(width, gap, track, filled)`; one ring is `.arc(share, width, tone, track)`.
#[track_caller]
pub fn rings(
    modifier: Modifier,
    width: f32,
    gap: f32,
    track: impl Into<Tone>,
    filled: &[(f32, Tone)],
) {
    let track = theme::color(track);
    let filled: Vec<(f32, rustle::Color)> = filled
        .iter()
        .map(|(share, tone)| (share.clamp(0.0, 1.0), theme::color(*tone)))
        .collect();
    rustle::canvas(modifier, move |canvas| {
        let side = canvas.width().min(canvas.height());
        let (x, y) = (canvas.width() / 2.0, canvas.height() / 2.0);
        for (index, (share, color)) in filled.iter().enumerate() {
            let radius = side / 2.0 - index as f32 * (width + gap);
            if radius - width < 0.0 {
                break;
            }
            canvas.stroke_circle(x, y, radius, width, track);
            if *share > 0.0 {
                canvas.arc(x, y, radius, width, 0.0, *share, *color);
            }
        }
    });
}

/// Emits one ring `width` pixels thick, `share` of it filled clockwise from
/// the top in `tone` and the rest in `track`: `.arc(share, width, tone, track)`.
#[track_caller]
pub fn arc(
    modifier: Modifier,
    share: f32,
    width: f32,
    tone: impl Into<Tone>,
    track: impl Into<Tone>,
) {
    rings(modifier, width, 0.0, track, &[(share, tone.into())]);
}

/// Emits a spinner `steps` of the spacing scale across in `tone`: an open
/// ring turning once a second, read from the frame's clock when painted, so
/// it turns without composing: `glyph(IconName::Loading, steps, tone)` with `animate_spin()`.
#[track_caller]
pub fn spinner(steps: f32, tone: impl Into<Tone>) {
    let side = sp(steps);
    let color = theme::color(tone);
    rustle::canvas(Modifier::new().size(side, side), move |canvas| {
        let now = frame::clock_ms();
        frame::animate();
        let turned = (now % TURN_MS) / TURN_MS * 360.0;
        let thickness = (side * 16.0 / 256.0).max(1.0);
        let radius = side * 104.0 / 256.0;
        let (x, y) = (side / 2.0, side / 2.0);
        canvas.arc(x, y, radius, thickness, turned + 25.0, 0.86, color);
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
