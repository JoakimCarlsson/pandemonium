//! A small square box that is ticked, empty or partly filled, such as the
//! box that stages a changed file or a whole group of them.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::size::Size;
use rustle::color_scheme;

/// How a checkbox is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckboxVariant {
    /// Filled with the accent colour when ticked.
    #[default]
    Accent,
    /// Filled with the muted text colour when ticked, for quiet lists.
    Quiet,
}

/// What a checkbox shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    /// Empty.
    #[default]
    Off,
    /// Ticked.
    On,
    /// Partly ticked, for a group where only some members are.
    Partial,
}

/// A checkbox with a variant and a size.
#[derive(Clone)]
pub struct Checkbox {
    /// What it shows.
    state: CheckState,
    /// How it is drawn.
    variant: CheckboxVariant,
    /// Its size.
    size: Size,
    /// What a click does.
    on_toggle: Option<Rc<dyn Fn()>>,
}

impl Checkbox {
    /// A checkbox showing `state`.
    pub fn new(state: CheckState) -> Self {
        Self {
            state,
            variant: CheckboxVariant::Accent,
            size: Size::Sm,
            on_toggle: None,
        }
    }

    /// A checkbox ticked when `checked`.
    pub fn checked(checked: bool) -> Self {
        Self::new(if checked {
            CheckState::On
        } else {
            CheckState::Off
        })
    }

    /// The checkbox in `variant`.
    pub fn variant(mut self, variant: CheckboxVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The checkbox at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The checkbox calling `on_toggle` when clicked.
    pub fn on_toggle(mut self, on_toggle: impl Fn() + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }

    /// Emits the checkbox.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let hovered = remember(|| false);
        let (side, glyph) = match self.size {
            Size::Xs => (12.0, 9.0),
            Size::Sm => (14.0, 10.0),
            Size::Md => (16.0, 12.0),
            Size::Lg => (18.0, 13.0),
        };
        let filled = match self.variant {
            CheckboxVariant::Accent => scheme.primary,
            CheckboxVariant::Quiet => scheme.on_surface_variant,
        };
        let (fill, border) = match self.state {
            CheckState::Off if hovered.get() => (Color::TRANSPARENT, scheme.on_surface_variant),
            CheckState::Off => (Color::TRANSPARENT, extended.subtle),
            CheckState::On | CheckState::Partial => (filled, filled),
        };
        let mut modifier = Modifier::new()
            .size(side, side)
            .background_rounded(fill, 3.0)
            .border(1.0, border, 3.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            })
            .cursor(CursorIcon::Pointer);
        if let Some(on_toggle) = self.on_toggle {
            modifier = modifier.clickable(move || on_toggle());
        }
        let state = self.state;
        stack_aligned(modifier, BoxAlignment::Center, move || match state {
            CheckState::On => icon(phosphor::CHECK, glyph, scheme.on_primary),
            CheckState::Partial => icon(phosphor::MINUS, glyph, scheme.on_primary),
            CheckState::Off => {}
        });
    }
}
