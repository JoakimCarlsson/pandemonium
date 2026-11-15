//! A button with an optional icon and a label: lit under the pointer with a
//! short colour transition, faint with a not-allowed cursor while disabled,
//! with a turning glyph while busy and a tooltip while the pointer rests.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::focus_ring::remember_focus_ring;
use crate::label::Label;
use crate::size::Size;
use crate::spinner::spinning_icon;
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
    /// A hairline outline with no fill.
    Outline,
    /// Underlined text, like a link.
    Link,
}

/// How long a button's colours take to follow the pointer, in milliseconds.
const FADE: u64 = 120;

/// How opaque a disabled button is drawn.
const DISABLED_ALPHA: f32 = 0.4;

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
    /// Whether its glyph turns, while what it does is under way.
    spinning: bool,
    /// What the label shows while the pointer rests on it.
    tooltip: Option<String>,
    /// Whether it is drawn chosen, as one of a set.
    selected: bool,
    /// Whether its ends are fully rounded.
    round: bool,
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
            spinning: false,
            tooltip: None,
            selected: false,
            round: false,
        }
    }

    /// The button with its glyph turning while `spinning`, for work under way.
    pub fn spinning(mut self, spinning: bool) -> Self {
        self.spinning = spinning;
        self
    }

    /// The button showing `tooltip` in a label while the pointer rests on it.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// The button drawn chosen when `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The button with fully rounded ends when `round`.
    pub fn round(mut self, round: bool) -> Self {
        self.round = round;
        self
    }

    /// The button leading with `icon`.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The button leading with `icon` when `shown`.
    pub fn icon_when(mut self, shown: bool, icon: Icon) -> Self {
        if shown {
            self.icon = Some(icon);
        }
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
            Size::Sm => (26.0, 8.0, 14.0),
            Size::Md => (32.0, 12.0, 16.0),
            Size::Lg => (38.0, 16.0, 16.0),
        };
        let (height, padding) = (crate::row_height(height), crate::spacing(padding));
        let (fill, tint, edge) = self.look(lit);
        let fill = animate_color_as_state(fill, tween(FADE)).get();
        let tint = animate_color_as_state(tint, tween(FADE)).get();
        let ring = remember_focus_ring();
        let radius = if self.round { height / 2.0 } else { 6.0 };
        let outline = if ring.shown() { scheme.primary } else { edge };
        let mut modifier = self
            .modifier
            .height(height)
            .background_rounded(fill, radius)
            .border(if ring.shown() { 1.5 } else { 1.0 }, outline, radius)
            .padding_xy(padding, 0.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if self.disabled {
            modifier = modifier
                .alpha(DISABLED_ALPHA)
                .cursor(CursorIcon::NotAllowed);
        } else if let Some(on_click) = self.on_click.clone() {
            let pressed = on_click.clone();
            let clicked = ring.clone();
            modifier = ring.focusable(
                modifier.cursor(CursorIcon::Pointer).clickable(move || {
                    clicked.clicked();
                    on_click()
                }),
                move || pressed(),
            );
        }
        let spinning = self.spinning;
        let link = self.variant == ButtonVariant::Link;
        let (lead, label, trailing) = (self.icon, self.label, self.trailing_icon);
        let body = move || {
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
                            if let Some(glyph_icon) = &lead {
                                if spinning && trailing.is_none() {
                                    spinning_icon(glyph_icon.clone(), glyph, tint);
                                } else {
                                    icon(glyph_icon.clone(), glyph, tint);
                                }
                            }
                            if !label.is_empty() {
                                let mut text = Label::new(label.clone()).colour(tint);
                                if link {
                                    text = text.underline();
                                }
                                text.show();
                            }
                            if let Some(glyph_icon) = &trailing {
                                if spinning {
                                    spinning_icon(glyph_icon.clone(), glyph - 2.0, tint);
                                } else {
                                    icon(glyph_icon.clone(), glyph - 2.0, tint);
                                }
                            }
                        },
                    );
                },
            );
        };
        match &self.tooltip {
            Some(text) => tooltip(text, body),
            None => body(),
        }
    }

    /// The fill, the colour of the words and glyphs, and the outline of the button, lit under the pointer when `lit`.
    fn look(&self, lit: bool) -> (Color, Color, Color) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let chosen = self.selected && lit;
        match self.variant {
            ButtonVariant::Primary if lit => {
                (extended.accent_hover, scheme.on_primary, Color::TRANSPARENT)
            }
            ButtonVariant::Primary => (scheme.primary, scheme.on_primary, Color::TRANSPARENT),
            ButtonVariant::Danger => (scheme.error, scheme.on_error, Color::TRANSPARENT),
            ButtonVariant::Secondary if lit || chosen => (
                scheme.surface_container_highest,
                scheme.on_surface,
                Color::TRANSPARENT,
            ),
            ButtonVariant::Secondary => (
                scheme.surface_container_high,
                scheme.on_surface,
                Color::TRANSPARENT,
            ),
            ButtonVariant::Outline => (
                if lit {
                    scheme.surface_container_high
                } else {
                    Color::TRANSPARENT
                },
                scheme.on_surface,
                if self.selected {
                    scheme.on_surface_variant
                } else {
                    scheme.outline_variant
                },
            ),
            ButtonVariant::Link => (
                Color::TRANSPARENT,
                if lit {
                    scheme.on_surface
                } else {
                    scheme.primary
                },
                Color::TRANSPARENT,
            ),
            ButtonVariant::Ghost if lit || self.selected => (
                if self.selected {
                    scheme.secondary_container
                } else {
                    scheme.surface_container_high
                },
                scheme.on_surface,
                Color::TRANSPARENT,
            ),
            ButtonVariant::Ghost => (
                Color::TRANSPARENT,
                scheme.on_surface_variant,
                Color::TRANSPARENT,
            ),
        }
    }
}
