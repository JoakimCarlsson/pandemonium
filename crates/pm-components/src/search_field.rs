//! A compact field with a magnifier, as in the top bar.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a search field is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchFieldVariant {
    /// A raised surface with a hairline border.
    #[default]
    Outlined,
    /// No fill until hovered.
    Ghost,
}

/// A search field with a variant and a size, which opens the command centre
/// when clicked.
#[derive(Clone)]
pub struct SearchField {
    /// The text shown, such as the open session's title.
    placeholder: String,
    /// How it is drawn.
    variant: SearchFieldVariant,
    /// Its size.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
}

impl SearchField {
    /// A field reading `placeholder`.
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            placeholder: placeholder.into(),
            variant: SearchFieldVariant::Outlined,
            size: Size::Sm,
            modifier: Modifier::new(),
            on_click: None,
        }
    }

    /// The field in `variant`.
    pub fn variant(mut self, variant: SearchFieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The field at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The field laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The field calling `on_click` when clicked.
    pub fn on_click(mut self, on_click: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// Emits the field.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let hovered = remember(|| false);
        let height = match self.size {
            Size::Xs => 22.0,
            Size::Sm => 26.0,
            Size::Md => 30.0,
            Size::Lg => 36.0,
        };
        let fill = match (self.variant, hovered.get()) {
            (_, true) => scheme.surface_container_high,
            (SearchFieldVariant::Outlined, false) => scheme.surface_container,
            (SearchFieldVariant::Ghost, false) => Color::TRANSPARENT,
        };
        let mut modifier = self
            .modifier
            .height(height)
            .background_rounded(fill, 6.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if self.variant == SearchFieldVariant::Outlined {
            modifier = modifier.border(1.0, scheme.outline_variant, 6.0);
        }
        if let Some(on_click) = self.on_click.clone() {
            modifier = modifier
                .cursor(CursorIcon::Pointer)
                .clickable(move || on_click());
        }
        let placeholder = self.placeholder;
        row_arranged(
            modifier.padding_xy(12.0, 0.0),
            Arrangement::SpacedBy(8.0),
            Alignment::Center,
            move || {
                icon(phosphor::MAGNIFYING_GLASS, 13.0, scheme.on_surface_variant);
                Label::new(placeholder)
                    .variant(LabelVariant::Muted)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
            },
        );
    }
}
