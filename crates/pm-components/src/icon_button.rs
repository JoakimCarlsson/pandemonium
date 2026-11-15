//! A square button holding one icon: lit under the pointer with a short
//! colour transition, faint with a not-allowed cursor while disabled, and
//! turning while busy.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::focus_ring::remember_focus_ring;
use crate::size::Size;
use crate::spinner::spinning_icon;
use rustle::color_scheme;

/// How an icon button is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconButtonVariant {
    /// No fill until hovered.
    #[default]
    Ghost,
    /// A filled surface.
    Filled,
    /// The accent fill, for the primary action.
    Primary,
}

/// A square button with an icon, a variant and a size.
#[derive(Clone)]
pub struct IconButton {
    /// The glyph.
    icon: Icon,
    /// How it is drawn.
    variant: IconButtonVariant,
    /// Its size.
    size: Size,
    /// A colour replacing the variant's icon colour.
    tint: Option<Color>,
    /// Whether it shows as switched on.
    active: bool,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
    /// What the label shown while the pointer rests on it says, when it shows one.
    tooltip: Option<String>,
    /// Whether it is faint and ignores clicks.
    disabled: bool,
    /// Whether its glyph turns, while what it does is under way.
    spinning: bool,
}

/// How long an icon button's colours take to follow the pointer, in milliseconds.
const FADE: u64 = 120;

impl IconButton {
    /// A ghost button showing `icon`.
    pub fn new(icon: Icon) -> Self {
        Self {
            icon,
            variant: IconButtonVariant::Ghost,
            size: Size::Sm,
            tint: None,
            active: false,
            on_click: None,
            tooltip: None,
            disabled: false,
            spinning: false,
        }
    }

    /// The button faint and ignoring clicks when `disabled`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The button with its glyph turning while `spinning`.
    pub fn spinning(mut self, spinning: bool) -> Self {
        self.spinning = spinning;
        self
    }

    /// The button showing `tooltip` in a label while the pointer rests on it.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// The button in `variant`.
    pub fn variant(mut self, variant: IconButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The button at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The icon painted `tint`.
    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = Some(tint);
        self
    }

    /// The button shown switched on when `active`.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// The button calling `on_click` when clicked.
    pub fn on_click(mut self, on_click: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// Emits the button.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let hovered = remember(|| false);
        let lit = hovered.get() && !self.disabled;
        let (side, glyph) = match self.size {
            Size::Xs => (20.0, 12.0),
            Size::Sm => (28.0, 16.0),
            Size::Md => (32.0, 16.0),
            Size::Lg => (36.0, 16.0),
        };
        let fill = match self.variant {
            IconButtonVariant::Primary if lit => extended_colors().accent_hover,
            IconButtonVariant::Primary => scheme.on_surface_variant,
            IconButtonVariant::Filled => scheme.surface_container_highest,
            IconButtonVariant::Ghost if self.active || lit => scheme.surface_container_high,
            IconButtonVariant::Ghost => Color::TRANSPARENT,
        };
        let tint = self.tint.unwrap_or(match self.variant {
            IconButtonVariant::Primary => scheme.background,
            _ if self.active || lit => scheme.on_surface,
            _ => scheme.on_surface_variant,
        });
        let fill = animate_color_as_state(fill, tween(FADE)).get();
        let tint = animate_color_as_state(tint, tween(FADE)).get();
        let radius = if self.variant == IconButtonVariant::Primary {
            side / 2.0
        } else {
            5.0
        };
        let ring_state = remember_focus_ring();
        let ring = if ring_state.shown() {
            scheme.primary
        } else {
            Color::TRANSPARENT
        };
        let mut modifier = Modifier::new()
            .size(side, side)
            .background_rounded(fill, radius)
            .border(1.5, ring, radius)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if self.disabled {
            modifier = modifier.alpha(0.4).cursor(CursorIcon::NotAllowed);
        } else if let Some(on_click) = self.on_click {
            modifier = modifier.cursor(CursorIcon::Pointer);
            let pressed = on_click.clone();
            let clicked = ring_state.clone();
            modifier = ring_state.focusable(
                modifier.clickable(move || {
                    clicked.clicked();
                    on_click()
                }),
                move || pressed(),
            );
        }
        let icon_glyph = self.icon;
        let spinning = self.spinning;
        let button = move || {
            stack_aligned(modifier, BoxAlignment::Center, move || {
                if spinning {
                    spinning_icon(icon_glyph, glyph, tint);
                } else {
                    icon(icon_glyph, glyph, tint);
                }
            });
        };
        match &self.tooltip {
            Some(label) => tooltip(label, button),
            None => button(),
        }
    }
}
