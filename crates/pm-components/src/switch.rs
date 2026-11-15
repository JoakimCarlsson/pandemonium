//! A switch that turns something on or off.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::size::Size;
use rustle::color_scheme;

/// How a switch is drawn while it is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwitchVariant {
    /// The accent fill.
    #[default]
    Accent,
    /// The success fill, for something running.
    Success,
}

/// A switch with a variant and a size.
#[derive(Clone)]
pub struct Switch {
    /// Whether it is on.
    on: bool,
    /// How it is drawn while on.
    variant: SwitchVariant,
    /// Its size.
    size: Size,
    /// Whether it is greyed out and ignores clicks.
    disabled: bool,
    /// What a click does, given whether it is to be on.
    on_change: Option<Rc<dyn Fn(bool)>>,
}

impl Switch {
    /// A switch standing `on`.
    pub fn new(on: bool) -> Self {
        Self {
            on,
            variant: SwitchVariant::Accent,
            size: Size::Sm,
            disabled: false,
            on_change: None,
        }
    }

    /// The switch in `variant`.
    pub fn variant(mut self, variant: SwitchVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The switch at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The switch greyed out when `disabled`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The switch calling `on_change` with the state a click asks for.
    pub fn on_change(mut self, on_change: impl Fn(bool) + 'static) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }

    /// The width the switch takes at `size`.
    pub fn width(size: Size) -> f32 {
        Self::metrics(size).0
    }

    /// The width, height and inset of the knob of a switch at `size`.
    fn metrics(size: Size) -> (f32, f32, f32) {
        match size {
            Size::Xs => (26.0, 14.0, 2.0),
            Size::Sm => (32.0, 18.0, 2.0),
            Size::Md => (36.0, 20.0, 2.0),
            Size::Lg => (44.0, 24.0, 3.0),
        }
    }

    /// Emits the switch.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let (width, height, inset) = Self::metrics(self.size);
        let knob = height - inset * 2.0;
        let track = match (self.on, self.variant) {
            (false, _) => scheme.surface_container_highest,
            (true, SwitchVariant::Accent) => scheme.primary,
            (true, SwitchVariant::Success) => extended.success,
        };
        let knob_colour = if self.on {
            scheme.on_primary
        } else {
            scheme.on_surface_variant
        };
        let alpha = if self.disabled { 0.5 } else { 1.0 };
        let ring = crate::focus_ring::remember_focus_ring();
        let outline = if ring.shown() {
            scheme.primary
        } else {
            Color::TRANSPARENT
        };
        let mut modifier = Modifier::new()
            .size(width, height)
            .alpha(alpha)
            .background_rounded(track, height / 2.0)
            .border(1.5, outline, height / 2.0)
            .padding(inset);
        if let (Some(on_change), false) = (self.on_change, self.disabled) {
            let on = self.on;
            let pressed = on_change.clone();
            let clicked = ring.clone();
            modifier = ring.focusable(
                modifier.cursor(CursorIcon::Pointer).clickable(move || {
                    clicked.clicked();
                    on_change(!on)
                }),
                move || pressed(!on),
            );
        }
        let alignment = if self.on {
            BoxAlignment::CenterEnd
        } else {
            BoxAlignment::CenterStart
        };
        stack_with(modifier, move || {
            spacer(
                Modifier::new()
                    .align(alignment)
                    .size(knob, knob)
                    .background_rounded(knob_colour, knob / 2.0),
            );
        });
    }
}
