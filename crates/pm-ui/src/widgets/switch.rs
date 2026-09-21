//! The switch: a two-state track and knob.

use pm_gfx::{Quad, Rect, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Style, Styled};

/// Width of the switch track.
const TRACK_WIDTH: f32 = 34.0;

/// Height of the switch track.
const TRACK_HEIGHT: f32 = 20.0;

/// Space between the knob and the track on every side.
const KNOB_INSET: f32 = 3.0;

/// A two-state track and knob that sends `message` when it is flipped.
pub struct Switch<M> {
    /// Whether the switch is on.
    on: bool,
    /// What flipping the switch sends.
    message: M,
    /// How the switch is sized; it has a fixed size by default.
    style: Style,
}

/// A switch in state `on` that sends `message` when it is flipped.
pub fn switch<M>(on: bool, message: M) -> Switch<M> {
    Switch {
        on,
        message,
        style: Style::default(),
    }
}

impl<M> Styled for Switch<M> {
    /// How the switch is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: Clone> Element<M> for Switch<M> {
    /// How the switch is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Reports the fixed size of the track.
    fn measure(&mut self, _available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        Size::new(TRACK_WIDTH, TRACK_HEIGHT)
    }

    /// Registers the press target, then paints the track and the knob.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let interaction = cx.interactive(bounds, self.message.clone());
        let theme = *cx.theme();

        let track = match (self.on, interaction.hovered) {
            (true, true) => theme.colors.accent_hover,
            (true, false) => theme.colors.accent,
            (false, true) => theme.colors.surface_hover,
            (false, false) => theme.colors.surface,
        };
        let border = if interaction.focused {
            theme.colors.border_focused
        } else if self.on {
            theme.colors.accent
        } else {
            theme.colors.border
        };

        cx.quad(
            Quad::filled(bounds, track)
                .corner_radius(theme.radius.full)
                .border(1.0, border),
        );

        let diameter = bounds.size.height - KNOB_INSET * 2.0;
        let left = if self.on {
            bounds.right() - KNOB_INSET - diameter
        } else {
            bounds.left() + KNOB_INSET
        };
        let knob = if self.on {
            theme.colors.text_on_accent
        } else {
            theme.colors.text_subtle
        };

        cx.quad(
            Quad::filled(
                Rect::from_xywh(left, bounds.top() + KNOB_INSET, diameter, diameter),
                knob,
            )
            .corner_radius(theme.radius.full),
        );
    }
}
