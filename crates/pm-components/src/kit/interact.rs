//! Hover, press, focus and group-hover styles, resolved when painting.
//!
//! The old utilities `bg(..).hover()`, `bg(..).active()`, `ring(2).focus()`
//! and `bg(..).group_hover()` were resolved against the pointer and the
//! focused id each frame. Here they are drawing closures that see the
//! area's [`rustle::Interaction`] when the interface is painted, so moving
//! the pointer or the focus repaints without composing. Focus is the host's:
//! [`Looks::focus_of`] and [`Hovered::focus_ring`] read the focused
//! [`FocusId`] the host set in the [`Frame`](crate::kit::Frame).

use rustle::{Canvas, Interaction, Modifier};

use crate::kit::focus::FocusId;
use crate::kit::frame;
use crate::kit::theme::{self, Radius, Role, Tone};

/// The fills of an area by its interaction state: at rest, under the
/// pointer, pressed, focused and while the group around it is hovered.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Looks {
    /// The fill at rest: `bg(tone)`.
    idle: Option<Tone>,
    /// The fill under the pointer: `bg(tone).hover()`.
    hover: Option<Tone>,
    /// The fill while pressed: `bg(tone).active()`.
    active: Option<Tone>,
    /// The fill while focused: `bg(tone).focus()`.
    focus: Option<Tone>,
    /// The fill while the group around it is hovered: `bg(tone).group_hover()`.
    group: Option<Tone>,
    /// How round its corners are.
    radius: Radius,
}

impl Looks {
    /// No fill in any state, square corners.
    pub fn new() -> Self {
        Self::default()
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

    /// Filled with `tone` while the group around it is hovered.
    pub fn group(mut self, tone: impl Into<Tone>) -> Self {
        self.group = Some(tone.into());
        self
    }

    /// Corners rounded by `radius`.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// The fill for `interaction`: pressed, else hovered, else group-hovered, else focused, else at rest.
    fn pick(&self, interaction: Interaction) -> Option<Tone> {
        let pressed = interaction.pressed.then_some(self.active).flatten();
        let hovered = interaction.hovered.then_some(self.hover).flatten();
        let grouped = interaction.group_hovered.then_some(self.group).flatten();
        let focused = interaction.focused.then_some(self.focus).flatten();
        pressed.or(hovered).or(grouped).or(focused).or(self.idle)
    }
}

/// Modifiers that style an area by its interaction state.
pub trait Hovered: Sized {
    /// Fills the area as `looks` says for its state when painted.
    fn looks(self, looks: Looks) -> Self;
    /// Like [`looks`](Self::looks), the area counting as focused while `id` holds keyboard focus.
    fn looks_of(self, looks: Looks, id: FocusId) -> Self;
    /// Counts the area as focused while `id` holds keyboard focus, for the
    /// interactive drawing after this modifier: `.focused_with(id)` and the
    /// `focus()` variants. Put it before them.
    fn focus_of(self, id: FocusId) -> Self;
    /// A ring `width` pixels wide in the focus role around the area while `id`
    /// holds keyboard focus, drawn over it inside corners rounded by `radius`:
    /// `ring(2).focus()` with `ring_color(Role::Focused).focus()`.
    fn focus_ring(self, id: FocusId, width: f32, radius: Radius) -> Self;
    /// A one pixel outline in `idle`, or in `focused` while `id` holds keyboard
    /// focus, inside corners rounded by `radius`: `border()`,
    /// `border_color(idle)`, `border_color(focused).focus()`.
    fn stroke_by_focus(
        self,
        id: FocusId,
        idle: impl Into<Tone>,
        focused: impl Into<Tone>,
        radius: Radius,
    ) -> Self;
    /// Makes the area a group: what is inside sees the pointer over it as group hover: `.group()`.
    fn group(self) -> Self;
    /// Draws the content only while the group around it is hovered or `id`
    /// holds focus: `opacity(0)`, `opacity(100).group_hover()`, `opacity(100).focus()`.
    fn shown_on_group_hover(self, id: Option<FocusId>) -> Self;
    /// Draws on a canvas behind the content with the area's interaction state,
    /// for styles the other helpers do not cover, such as a border that
    /// changes colour under the pointer.
    fn styled_by(self, draw: impl Fn(&mut Canvas, Interaction) + 'static) -> Self;
}

impl Hovered for Modifier {
    fn looks(self, looks: Looks) -> Self {
        let theme = theme::theme();
        let corner = looks.radius.px();
        self.draw_behind_interactive(move |canvas, interaction| {
            let Some(tone) = looks.pick(interaction) else {
                return;
            };
            let color = theme::color_in(&theme, tone);
            if color.a == 0 {
                return;
            }
            let (width, height) = (canvas.width(), canvas.height());
            let corner = corner.min(width.min(height) / 2.0);
            canvas.rounded_rect(0.0, 0.0, width, height, corner, color);
        })
    }

    fn looks_of(self, looks: Looks, id: FocusId) -> Self {
        self.focus_of(id).looks(looks)
    }

    fn focus_of(self, id: FocusId) -> Self {
        self.interaction_focused_with(move || frame::is_focused(id))
    }

    fn focus_ring(self, id: FocusId, width: f32, radius: Radius) -> Self {
        let color = theme::role(Role::Focused);
        let corner = radius.px();
        self.interaction_focused_with(move || frame::is_focused(id))
            .draw_over_interactive(move |canvas, interaction| {
                if !interaction.focused {
                    return;
                }
                let (w, h) = (canvas.width(), canvas.height());
                let corner = corner.min(w.min(h) / 2.0);
                canvas.stroke_rect(
                    -width,
                    -width,
                    w + 2.0 * width,
                    h + 2.0 * width,
                    corner + width,
                    width,
                    color,
                );
            })
    }

    fn stroke_by_focus(
        self,
        id: FocusId,
        idle: impl Into<Tone>,
        focused: impl Into<Tone>,
        radius: Radius,
    ) -> Self {
        let (idle, focused) = (theme::color(idle), theme::color(focused));
        let corner = radius.px();
        self.interaction_focused_with(move || frame::is_focused(id))
            .draw_behind_interactive(move |canvas, interaction| {
                let color = if interaction.focused { focused } else { idle };
                let (w, h) = (canvas.width(), canvas.height());
                let corner = corner.min(w.min(h) / 2.0);
                canvas.stroke_rect(0.0, 0.0, w, h, corner, 1.0, color);
            })
    }

    fn group(self) -> Self {
        self.interaction_group()
    }

    fn shown_on_group_hover(self, id: Option<FocusId>) -> Self {
        let focusing = match id {
            Some(id) => self.interaction_focused_with(move || frame::is_focused(id)),
            None => self,
        };
        focusing.draw_with_content_interactive(|canvas, interaction| {
            if interaction.group_hovered || interaction.focused {
                canvas.draw_content();
            }
        })
    }

    fn styled_by(self, draw: impl Fn(&mut Canvas, Interaction) + 'static) -> Self {
        self.draw_behind_interactive(draw)
    }
}
