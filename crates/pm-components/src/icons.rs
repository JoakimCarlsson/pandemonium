//! Icons: rustle's Phosphor drawings ([`Icon`], such as `phosphor::X` or
//! `phosphor::fill::X` for a selected state), tinted with a tone like text,
//! and artwork that arrives while the app runs, such as an extension's icon
//! or a file icon, loaded as an [`SvgPainter`] and drawn tinted by rustle.
//!
//! The panel drawings Phosphor lacks are its own turned half round:
//! [`panel_right`] is its sidebar with the panel on the right,
//! [`panel_bottom`] its browser window with the band along the bottom. Both
//! drawings are symmetric across the other axis, so half a turn mirrors them.
//! The spinner and the turning icon turn on an angle of rustle's infinite
//! transition, on the composition's frame clock.

use rustle::{Icon, Modifier, Path, SvgPainter, phosphor};

use crate::theme::{self, Tone, sp};

/// Emits a window with a panel along its right side, filled to show it is
/// open when `filled`, `steps` of the spacing scale on a side, in `tone`:
/// Phosphor's sidebar turned half round.
#[track_caller]
pub fn panel_right(filled: bool, steps: f32, tone: impl Into<Tone>) {
    let sidebar = if filled {
        phosphor::fill::SIDEBAR_SIMPLE
    } else {
        phosphor::SIDEBAR_SIMPLE
    };
    turned_icon(sidebar, steps, tone);
}

/// Emits a window with a panel along its bottom, `steps` of the spacing scale
/// on a side, in `tone`: Phosphor's browser window turned half round.
#[track_caller]
pub fn panel_bottom(steps: f32, tone: impl Into<Tone>) {
    turned_icon(phosphor::BROWSER, steps, tone);
}

/// Emits `icon` turned half round, `steps` of the spacing scale on a side, in `tone`.
#[track_caller]
fn turned_icon(icon: Icon, steps: f32, tone: impl Into<Tone>) {
    icon_with(Modifier::new().rotate(180.0), icon, sp(steps), tone);
}

/// Emits `icon` in a square `steps` of the spacing scale on a side, tinted
/// with `tone`: `glyph(icon, steps, tone)`, or a box sized `size(steps)` with `.icon(icon)`.
#[track_caller]
pub fn icon(icon: Icon, steps: f32, tone: impl Into<Tone>) {
    icon_with(Modifier::new(), icon, sp(steps), tone);
}

/// Emits `icon` in a square `pixels` logical pixels on a side, tinted with `tone`, with `modifier` around it.
#[track_caller]
pub fn icon_with(modifier: Modifier, icon: Icon, pixels: f32, tone: impl Into<Tone>) {
    path_with(modifier, icon, pixels, tone);
}

/// Emits the vector outline `path`, such as `phosphor::X`, in a square
/// `pixels` logical pixels on a side, tinted with `tone`, with `modifier` around it.
#[track_caller]
pub fn path_with(modifier: Modifier, path: Path, pixels: f32, tone: impl Into<Tone>) {
    let color = theme::color(tone);
    rustle::canvas(modifier.size(pixels, pixels), move |canvas| {
        let side = canvas.width().min(canvas.height());
        let (x, y) = (
            (canvas.width() - side) / 2.0,
            (canvas.height() - side) / 2.0,
        );
        canvas.path(&path, x, y, side, side, color);
    });
}

/// Emits the SVG artwork `art` in a square `pixels` logical pixels on a side,
/// tinted with `tone`, with `modifier` around it.
#[track_caller]
pub fn artwork_with(modifier: Modifier, art: &SvgPainter, pixels: f32, tone: impl Into<Tone>) {
    rustle::svg_icon(modifier, art, pixels, Some(theme::color(tone)));
}

/// How long a spinner takes to turn once.
const TURN: std::time::Duration = std::time::Duration::from_secs(1);

/// The angle, in degrees, of something turning once a [`TURN`] at an even
/// pace: an angle of rustle's infinite transition, on the composition's frame clock.
#[track_caller]
fn turning() -> rustle::State<f32> {
    rustle::remember_infinite_transition().animate_float(
        0.0,
        360.0,
        rustle::infinite_repeatable(
            rustle::AnimationSpec::Tween {
                duration: TURN,
                easing: rustle::Easing::Linear,
            },
            rustle::RepeatMode::Restart,
        ),
    )
}

/// Emits a spinner `steps` of the spacing scale across in `tone`: rustle's
/// dashed ring, the loading icon, turning once a second, its angle read when
/// it is painted, so it turns without composing again.
#[track_caller]
pub fn spinner(steps: f32, tone: impl Into<Tone>) {
    let turned = turning();
    rustle::dashed_spinner(sp(steps), theme::color(tone), move || turned.get());
}

/// Emits `icon` turning once a second, `steps` of the spacing scale across,
/// in `tone`, for an icon other than the loading ring, which is [`spinner`]:
/// its angle read when it is painted, so it turns without composing again.
#[track_caller]
pub fn spinning_icon(icon: Icon, steps: f32, tone: impl Into<Tone>) {
    let turned = turning();
    icon_with(
        Modifier::new().rotate_with(move || turned.get()),
        icon,
        sp(steps),
        tone,
    );
}
