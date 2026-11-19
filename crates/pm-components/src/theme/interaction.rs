//! The theme's colours for the states of an interactive area, handed to
//! rustle's interaction support: [`InteractionTones`] resolves to the
//! [`rustle::InteractionColors`] that
//! [`Modifier::interaction_background`](rustle::Modifier::interaction_background)
//! fills with, and [`focus_ring`] and [`outline_by_focus`] are drawing closures
//! for [`Modifier::draw_over_interactive`](rustle::Modifier::draw_over_interactive)
//! and [`Modifier::draw_behind_interactive`](rustle::Modifier::draw_behind_interactive).
//! Rustle resolves the state when it paints, so moving the pointer repaints
//! without composing.

use rustle::{Canvas, Color, Interaction, InteractionColors};

use super::{Radius, Role, Tone, color};

/// The fills of an area by its interaction state, as tones of the theme: at
/// rest, under the pointer, pressed and focused, with its corner radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InteractionTones {
    /// The fill at rest.
    idle: Option<Tone>,
    /// The fill under the pointer.
    hover: Option<Tone>,
    /// The fill while pressed.
    active: Option<Tone>,
    /// The fill while focused.
    focus: Option<Tone>,
    /// How round its corners are.
    radius: Radius,
}

impl InteractionTones {
    /// No fill in any state, square corners.
    pub fn new() -> Self {
        Self {
            radius: Radius::None,
            ..Self::default()
        }
    }

    /// Filled with `tone` at rest.
    pub fn idle(mut self, tone: impl Into<Tone>) -> Self {
        self.idle = Some(tone.into());
        self
    }

    /// Filled with `tone` under the pointer.
    pub fn hover(mut self, tone: impl Into<Tone>) -> Self {
        self.hover = Some(tone.into());
        self
    }

    /// Filled with `tone` while pressed.
    pub fn active(mut self, tone: impl Into<Tone>) -> Self {
        self.active = Some(tone.into());
        self
    }

    /// Filled with `tone` while focused.
    pub fn focus(mut self, tone: impl Into<Tone>) -> Self {
        self.focus = Some(tone.into());
        self
    }

    /// Corners rounded by `radius`.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// The colours in the theme being composed with: a state without a tone of
    /// its own takes the one it falls back to, pressed to hovered, hovered to
    /// focused, focused to at rest, and no tone at all is transparent.
    pub fn colors(&self) -> InteractionColors {
        let resolved = |tone: Option<Tone>| tone.map_or(Color::TRANSPARENT, color);
        let focused = self.focus.or(self.idle);
        let hovered = self.hover.or(focused);
        InteractionColors {
            idle: resolved(self.idle),
            hovered: resolved(hovered),
            pressed: resolved(self.active.or(hovered)),
            focused: resolved(focused),
            corner_radius: self.radius.px(),
        }
    }
}

/// Draws a ring `width` pixels wide in the focus role around the area while it
/// counts as focused, outside corners rounded by `radius`.
pub fn focus_ring(width: f32, radius: Radius) -> impl Fn(&mut Canvas, Interaction) + 'static {
    let ring = color(Role::Focused);
    let corner = radius.px();
    move |canvas, interaction| {
        if !interaction.focused {
            return;
        }
        let (across, down) = (canvas.width(), canvas.height());
        let corner = corner.min(across.min(down) / 2.0);
        canvas.stroke_rect(
            -width,
            -width,
            across + 2.0 * width,
            down + 2.0 * width,
            corner + width,
            width,
            ring,
        );
    }
}

/// Draws a one pixel outline in `idle`, or in `focused` while the area counts
/// as focused, inside corners rounded by `radius`; follow it with a pixel of padding.
pub fn outline_by_focus(
    idle: Color,
    focused: Color,
    radius: Radius,
) -> impl Fn(&mut Canvas, Interaction) + 'static {
    let corner = radius.px();
    move |canvas, interaction| {
        let line = if interaction.focused { focused } else { idle };
        let (across, down) = (canvas.width(), canvas.height());
        let corner = corner.min(across.min(down) / 2.0);
        canvas.stroke_rect(0.0, 0.0, across, down, corner, 1.0, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A state without a tone of its own falls back as the old hover styles did: pressed to
    /// hovered, hovered to focused, focused to at rest, and nothing to transparent.
    #[test]
    fn states_without_a_tone_fall_back() {
        let tones = InteractionTones::new()
            .idle(Role::SurfaceSelected)
            .hover(Role::SurfaceHover)
            .radius(Radius::Lg);
        let colors = tones.colors();
        assert_eq!(colors.idle, color(Role::SurfaceSelected));
        assert_eq!(colors.focused, color(Role::SurfaceSelected));
        assert_eq!(colors.hovered, color(Role::SurfaceHover));
        assert_eq!(colors.pressed, color(Role::SurfaceHover));
        assert_eq!(colors.corner_radius, 8.0);
        let bare = InteractionTones::new().active(Role::SurfaceActive).colors();
        assert_eq!(bare.idle, Color::TRANSPARENT);
        assert_eq!(bare.hovered, Color::TRANSPARENT);
        assert_eq!(bare.pressed, color(Role::SurfaceActive));
    }
}
