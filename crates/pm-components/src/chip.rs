//! A rounded chip with an optional icon and a label, filled while selected,
//! such as the agents on the welcome page.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::label::Label;
use crate::size::Size;
use rustle::color_scheme;

/// How a chip is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChipVariant {
    /// A hairline outline, filled while selected.
    #[default]
    Outline,
    /// A raised surface, accented while selected.
    Filled,
}

/// A chip with a variant and a size.
#[derive(Clone)]
pub struct Chip {
    /// Its text.
    label: String,
    /// Its glyph, before the text.
    icon: Option<Icon>,
    /// How it is drawn.
    variant: ChipVariant,
    /// Its size.
    size: Size,
    /// Whether it is the one chosen.
    selected: bool,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
    /// What the cross at its end does, when it has one.
    on_remove: Option<Rc<dyn Fn()>>,
}

impl Chip {
    /// An outlined chip reading `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            variant: ChipVariant::Outline,
            size: Size::Md,
            selected: false,
            modifier: Modifier::new(),
            on_click: None,
            on_remove: None,
        }
    }

    /// The chip leading with `icon`.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The chip in `variant`.
    pub fn variant(mut self, variant: ChipVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The chip at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The chip shown as chosen when `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The chip laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The chip calling `on_click` when clicked.
    pub fn on_click(mut self, on_click: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// The chip ending with a cross that calls `on_remove`.
    pub fn on_remove(mut self, on_remove: impl Fn() + 'static) -> Self {
        self.on_remove = Some(Rc::new(on_remove));
        self
    }

    /// Emits the chip.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let hovered = remember(|| false);
        let lit = hovered.get();
        let (height, padding, glyph) = match self.size {
            Size::Xs => (22.0, 8.0, 12.0),
            Size::Sm => (26.0, 10.0, 14.0),
            Size::Md => (32.0, 14.0, 16.0),
            Size::Lg => (38.0, 18.0, 16.0),
        };
        let (height, padding) = (crate::row_height(height), crate::spacing(padding));
        let (fill, edge, tint) = match (self.variant, self.selected) {
            (ChipVariant::Outline, true) => (
                scheme.surface_container_highest,
                extended.subtle,
                scheme.on_surface,
            ),
            (ChipVariant::Outline, false) if lit => (
                scheme.surface_container_high,
                scheme.outline_variant,
                scheme.on_surface,
            ),
            (ChipVariant::Outline, false) => (
                Color::TRANSPARENT,
                scheme.outline_variant,
                scheme.on_surface_variant,
            ),
            (ChipVariant::Filled, true) => (scheme.primary, scheme.primary, scheme.on_primary),
            (ChipVariant::Filled, false) if lit => (
                scheme.surface_container_highest,
                scheme.surface_container_highest,
                scheme.on_surface,
            ),
            (ChipVariant::Filled, false) => (
                scheme.surface_container_high,
                scheme.surface_container_high,
                scheme.on_surface_variant,
            ),
        };
        let radius = height / 2.0;
        let mut modifier = self
            .modifier
            .height(height)
            .background_rounded(fill, radius)
            .border(1.0, edge, radius)
            .padding_xy(padding, 0.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if let Some(on_click) = self.on_click.clone() {
            modifier = modifier
                .cursor(CursorIcon::Pointer)
                .clickable(move || on_click());
        }
        row_arranged(
            modifier,
            Arrangement::SpacedBy(crate::spacing(6.0)),
            Alignment::Center,
            move || {
                if let Some(glyph_icon) = &self.icon {
                    icon(glyph_icon.clone(), glyph, tint);
                }
                Label::new(self.label.clone()).colour(tint).show();
                if let Some(on_remove) = self.on_remove.clone() {
                    stack_aligned(
                        Modifier::new()
                            .size(glyph + 4.0, glyph + 4.0)
                            .cursor(CursorIcon::Pointer)
                            .clickable_plain(move || on_remove()),
                        BoxAlignment::Center,
                        || icon(phosphor::X, glyph - 2.0, tint),
                    );
                }
            },
        );
    }
}
