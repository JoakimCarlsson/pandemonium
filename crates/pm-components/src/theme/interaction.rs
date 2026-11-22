//! The theme's indications: how an interactive area shows the pointer over it,
//! a press and keyboard focus. [`InteractionTones`] and [`FocusRing`] are rustle
//! [`IndicationNodeFactory`]s resolved when the area is painted: the fills follow
//! the pointer and focus with rustle's interaction background, and the ring is
//! drawn outside the area's bounds while it holds focus, so neither composes nor
//! lays out again when the state changes. [`FocusOutline`] is a plain modifier, a
//! border whose colour the focus known in composition picks. [`Indicated`] gives
//! an area its remembered [`MutableInteractionSource`], fed by its focus, and
//! shows an indication for it.

use rustle::interaction_source::Interaction;
use rustle::{
    Color, FocusInteraction, Indication, IndicationNodeFactory, InteractionColors, InteractionId,
    InteractionSource, Modifier, MutableInteractionSource, remember_mutable_interaction_source,
};

use super::{Radius, Role, color};

/// The fills of an area by its interaction state: at rest, under the pointer,
/// pressed and focused, with its corner radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InteractionTones {
    /// The fill at rest.
    idle: Option<Color>,
    /// The fill under the pointer.
    hover: Option<Color>,
    /// The fill while pressed.
    active: Option<Color>,
    /// The fill while focused.
    focus: Option<Color>,
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

    /// Filled with `fill` at rest.
    pub fn idle(mut self, fill: impl Into<Color>) -> Self {
        self.idle = Some(fill.into());
        self
    }

    /// Filled with `fill` under the pointer.
    pub fn hover(mut self, fill: impl Into<Color>) -> Self {
        self.hover = Some(fill.into());
        self
    }

    /// Filled with `fill` while pressed.
    pub fn active(mut self, fill: impl Into<Color>) -> Self {
        self.active = Some(fill.into());
        self
    }

    /// Filled with `fill` while focused.
    pub fn focus(mut self, fill: impl Into<Color>) -> Self {
        self.focus = Some(fill.into());
        self
    }

    /// Corners rounded by `radius`.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// The colour of an area pressed, hovered and focused as they say: a state
    /// without a fill of its own takes the one it falls back to, pressed to
    /// hovered, hovered to focused, focused to at rest, and no fill at all is
    /// transparent.
    pub fn fill(&self, pressed: bool, hovered: bool, focused: bool) -> Color {
        let resolved = |fill: Option<Color>| fill.unwrap_or(Color::TRANSPARENT);
        let at_focus = self.focus.or(self.idle);
        let at_hover = self.hover.or(at_focus);
        if pressed {
            resolved(self.active.or(at_hover))
        } else if hovered {
            resolved(at_hover)
        } else if focused {
            resolved(at_focus)
        } else {
            resolved(self.idle)
        }
    }

    /// How round the corners of the fill are, in logical pixels.
    pub fn corner(&self) -> f32 {
        self.radius.px()
    }
}

impl IndicationNodeFactory for InteractionTones {
    /// Fills the area behind its content with rustle's
    /// [`Modifier::interaction_background`], in the colours [`InteractionTones::fill`]
    /// gives for the pointer over it and its press, and for the focus
    /// `interaction_source` reports.
    fn create(&self, interaction_source: &InteractionSource) -> Modifier {
        let focused = interaction_source.collect_is_focused_as_state();
        Modifier::new()
            .interaction_focused_with(move || focused.get())
            .interaction_background(InteractionColors {
                idle: self.fill(false, false, false),
                hovered: self.fill(false, true, false),
                pressed: self.fill(true, true, false),
                focused: self.fill(false, false, true),
                corner_radius: self.corner(),
            })
    }
}

/// A ring `width` pixels wide in the focus role drawn around the outside of an area
/// while it holds focus, outside corners rounded by `radius`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusRing {
    /// How wide the ring is, in logical pixels.
    pub width: f32,
    /// How round the corners of the area are.
    pub radius: Radius,
}

impl IndicationNodeFactory for FocusRing {
    /// Draws the ring over the area, `width` pixels outside its bounds with corners
    /// `radius + width`, while the focus `interaction_source` reports is held when the
    /// area is painted, and nothing otherwise; the area's size and layout never change.
    fn create(&self, interaction_source: &InteractionSource) -> Modifier {
        let focused = interaction_source.collect_is_focused_as_state();
        let width = self.width;
        let radius = self.radius.px();
        let ring = color(Role::Focused);
        Modifier::new()
            .interaction_focused_with(move || focused.get())
            .draw_over_interactive(move |canvas, now| {
                if now.focused {
                    let (across, down) = (canvas.width(), canvas.height());
                    let corner = radius.min(across.min(down) / 2.0);
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
            })
    }
}

/// A one pixel outline inside an area chosen by whether it holds focus.
pub trait FocusOutline: Sized {
    /// This modifier bordered one pixel wide in `focused_colour` when `focused` and in
    /// `idle` otherwise, inside corners rounded by `radius`; follow it with a pixel of
    /// padding.
    fn focus_outline(
        self,
        focused: bool,
        idle: Color,
        focused_colour: Color,
        radius: Radius,
    ) -> Self;
}

impl FocusOutline for Modifier {
    /// Borders the area with rustle's [`Modifier::border`] in the colour `focused` picks.
    fn focus_outline(
        self,
        focused: bool,
        idle: Color,
        focused_colour: Color,
        radius: Radius,
    ) -> Self {
        let line = if focused { focused_colour } else { idle };
        self.border(1.0, line, radius.px())
    }
}

/// Where an indicated area's focus comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focused {
    /// Rustle's keyboard focus, held by the first focusable inside the area.
    Rustle,
    /// Rustle's focus of the control the area stands for, as the view read it from the
    /// control's focus in the registry of [`crate::focus`]: held when `true`.
    Control(bool),
}

/// Showing an indication for an area from the focus it reports.
pub trait Indicated: Sized {
    /// This modifier with a remembered [`MutableInteractionSource`] told of its
    /// focus as `focused` says, and `indication` shown from it with
    /// [`Modifier::indication`].
    #[track_caller]
    fn indicated(self, indication: impl IndicationNodeFactory + 'static, focused: Focused) -> Self;
}

impl Indicated for Modifier {
    /// Feeds a remembered source with the focus `focused` names and shows
    /// `indication` from it.
    #[track_caller]
    fn indicated(self, indication: impl IndicationNodeFactory + 'static, focused: Focused) -> Self {
        let source = remember_mutable_interaction_source();
        let modifier = match focused {
            Focused::Rustle => {
                let changed = source.clone();
                self.on_focus_changed(move |now| set_focused(&changed, now))
            }
            Focused::Control(held) => {
                let changed = source.clone();
                rustle::side_effect(move || set_focused(&changed, held));
                self
            }
        };
        modifier.indication(&source, Some(&Indication::new(indication)))
    }
}

/// The ids of the interactions under way in `source` that `started` picks out.
fn under_way(
    source: &MutableInteractionSource,
    started: fn(&Interaction) -> Option<InteractionId>,
) -> Vec<InteractionId> {
    source
        .current_interactions()
        .iter()
        .filter_map(started)
        .collect()
}

/// The id of `interaction` when it is a focus that started.
fn started_focus(interaction: &Interaction) -> Option<InteractionId> {
    match interaction {
        Interaction::Focus(FocusInteraction::Focus { id }) => Some(*id),
        _ => None,
    }
}

/// Tells `source` the area holds focus when `focused` and does not otherwise,
/// emitting only a change.
fn set_focused(source: &MutableInteractionSource, focused: bool) {
    let held = under_way(source, started_focus);
    if focused && held.is_empty() {
        source.emit(FocusInteraction::Focus {
            id: InteractionId::next(),
        });
    } else if !focused {
        for focus in held {
            source.emit(FocusInteraction::Unfocus { focus });
        }
    }
}

/// The fills of a row of a list that is filled while `selected`, and fills under the pointer only while it is not.
pub fn selectable(selected: bool) -> InteractionTones {
    if selected {
        InteractionTones::new().idle(Role::SurfaceSelected)
    } else {
        InteractionTones::new().hover(Role::SurfaceHover)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A state without a fill of its own falls back as the old hover styles did: pressed to
    /// hovered, hovered to focused, focused to at rest, and nothing to transparent.
    #[test]
    fn states_without_a_tone_fall_back() {
        let tones = InteractionTones::new()
            .idle(Role::SurfaceSelected)
            .hover(Role::SurfaceHover)
            .radius(Radius::Lg);
        assert_eq!(
            tones.fill(false, false, false),
            color(Role::SurfaceSelected)
        );
        assert_eq!(tones.fill(false, false, true), color(Role::SurfaceSelected));
        assert_eq!(tones.fill(false, true, false), color(Role::SurfaceHover));
        assert_eq!(tones.fill(true, true, false), color(Role::SurfaceHover));
        assert_eq!(tones.corner(), 8.0);
        let bare = InteractionTones::new().active(Role::SurfaceActive);
        assert_eq!(bare.fill(false, false, false), Color::TRANSPARENT);
        assert_eq!(bare.fill(false, true, false), Color::TRANSPARENT);
        assert_eq!(bare.fill(true, true, false), color(Role::SurfaceActive));
    }

    /// Focus told to a source is emitted once and ended when it goes.
    #[test]
    fn focus_is_told_once() {
        let source = MutableInteractionSource::new();
        set_focused(&source, true);
        set_focused(&source, true);
        assert_eq!(source.current_interactions().len(), 1);
        set_focused(&source, false);
        assert!(source.current_interactions().is_empty());
    }
}
