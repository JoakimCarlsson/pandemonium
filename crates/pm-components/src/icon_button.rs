//! A square button holding one icon.

use std::rc::Rc;

use rustle::prelude::*;

use crate::size::Size;
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
}

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
        }
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
        let (side, glyph) = match self.size {
            Size::Xs => (20.0, 12.0),
            Size::Sm => (26.0, 15.0),
            Size::Md => (30.0, 16.0),
            Size::Lg => (34.0, 18.0),
        };
        let fill = match self.variant {
            IconButtonVariant::Primary => scheme.on_surface_variant,
            IconButtonVariant::Filled => scheme.surface_container_highest,
            IconButtonVariant::Ghost if self.active || hovered.get() => {
                scheme.surface_container_high
            }
            IconButtonVariant::Ghost => Color::TRANSPARENT,
        };
        let tint = self.tint.unwrap_or(match self.variant {
            IconButtonVariant::Primary => scheme.background,
            _ if self.active || hovered.get() => scheme.on_surface,
            _ => scheme.on_surface_variant,
        });
        let radius = if self.variant == IconButtonVariant::Primary {
            side / 2.0
        } else {
            5.0
        };
        let mut modifier = Modifier::new()
            .size(side, side)
            .background_rounded(fill, radius)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            })
            .cursor(CursorIcon::Pointer);
        if let Some(on_click) = self.on_click {
            modifier = modifier.clickable(move || on_click());
        }
        let icon_glyph = self.icon;
        stack_aligned(modifier, BoxAlignment::Center, move || {
            icon(icon_glyph, glyph, tint);
        });
    }
}
