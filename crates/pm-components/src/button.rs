//! A button with an optional icon and a label.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::Label;
use crate::size::Size;
use rustle::color_scheme;

/// How a button is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Text only, filled under the pointer.
    #[default]
    Ghost,
    /// A raised surface.
    Secondary,
    /// The accent fill.
    Primary,
    /// The danger fill, for throwing work away.
    Danger,
}

/// A button with a variant and a size.
#[derive(Clone)]
pub struct Button {
    /// Its text.
    label: String,
    /// Its glyph, before the text.
    icon: Option<Icon>,
    /// A glyph after the text, such as a chevron.
    trailing_icon: Option<Icon>,
    /// How it is drawn.
    variant: ButtonVariant,
    /// Its size.
    size: Size,
    /// Whether it is greyed out and ignores clicks.
    disabled: bool,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
}

impl Button {
    /// A ghost button reading `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            trailing_icon: None,
            variant: ButtonVariant::Ghost,
            size: Size::Sm,
            disabled: false,
            modifier: Modifier::new(),
            on_click: None,
        }
    }

    /// The button leading with `icon`.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The button ending in `icon`.
    pub fn trailing_icon(mut self, icon: Icon) -> Self {
        self.trailing_icon = Some(icon);
        self
    }

    /// The button in `variant`.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The button at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The button greyed out when `disabled`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The button laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
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
        let (height, padding, glyph) = match self.size {
            Size::Xs => (22.0, 6.0, 12.0),
            Size::Sm => (26.0, 8.0, 13.0),
            Size::Md => (32.0, 12.0, 14.0),
            Size::Lg => (38.0, 16.0, 16.0),
        };
        let (height, padding) = (crate::row_height(height), crate::spacing(padding));
        let (fill, tint) = match self.variant {
            ButtonVariant::Primary => (scheme.primary, scheme.on_primary),
            ButtonVariant::Danger => (scheme.error, scheme.on_primary),
            ButtonVariant::Secondary if lit => {
                (scheme.surface_container_highest, scheme.on_surface)
            }
            ButtonVariant::Secondary => (scheme.surface_container_high, scheme.on_surface_variant),
            ButtonVariant::Ghost if lit => (scheme.surface_container_high, scheme.on_surface),
            ButtonVariant::Ghost => (Color::TRANSPARENT, scheme.on_surface_variant),
        };
        let tint = if self.disabled {
            scheme.on_surface_variant
        } else {
            tint
        };
        let mut modifier = self
            .modifier
            .height(height)
            .background_rounded(fill, 6.0)
            .padding_xy(padding, 0.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if let (Some(on_click), false) = (self.on_click.clone(), self.disabled) {
            modifier = modifier
                .cursor(CursorIcon::Pointer)
                .clickable(move || on_click());
        }
        row_arranged(
            modifier,
            Arrangement::Center,
            Alignment::Center,
            move || {
                row_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(crate::spacing(6.0)),
                    Alignment::Center,
                    move || {
                        if let Some(glyph_icon) = &self.icon {
                            icon(glyph_icon.clone(), glyph, tint);
                        }
                        if !self.label.is_empty() {
                            Label::new(self.label.clone()).colour(tint).show();
                        }
                        if let Some(glyph_icon) = &self.trailing_icon {
                            icon(glyph_icon.clone(), glyph - 2.0, tint);
                        }
                    },
                );
            },
        );
    }
}
