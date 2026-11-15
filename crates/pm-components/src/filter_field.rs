//! A field the reader types into to narrow a list, with a magnifier and a
//! placeholder while it is empty.

use rustle::basic_text_field;

use crate::text_menu::TextMenu;
use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a filter field is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilterFieldVariant {
    /// The raised surface with a hairline border.
    #[default]
    Outlined,
    /// No fill and no border.
    Ghost,
}

/// A text field that narrows a list, with a variant and a size.
#[derive(Clone)]
pub struct FilterField {
    /// The faint text shown while nothing is typed.
    placeholder: String,
    /// How it is drawn.
    variant: FilterFieldVariant,
    /// Its size.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
}

impl FilterField {
    /// An outlined field reading `placeholder` while empty.
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            placeholder: placeholder.into(),
            variant: FilterFieldVariant::Outlined,
            size: Size::Md,
            modifier: Modifier::new(),
        }
    }

    /// The field in `variant`.
    pub fn variant(mut self, variant: FilterFieldVariant) -> Self {
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

    /// Emits the field and returns what is typed in it, trimmed and in
    /// lower case, ready to match against.
    #[track_caller]
    pub fn show(self) -> String {
        let scheme = color_scheme();
        let state = remember_text_field_state("");
        let height = match self.size {
            Size::Xs => 24.0,
            Size::Sm => 28.0,
            Size::Md => 34.0,
            Size::Lg => 40.0,
        };
        let height = crate::row_height(height);
        let mut modifier = self.modifier.height(height);
        if self.variant == FilterFieldVariant::Outlined {
            modifier = modifier
                .background_rounded(scheme.surface_container, 8.0)
                .border(1.0, scheme.outline_variant, 8.0);
        }
        let text = state.text();
        let empty = text.is_empty();
        let placeholder = self.placeholder;
        row_arranged(
            modifier.padding_xy(12.0, 0.0),
            Arrangement::SpacedBy(crate::spacing(8.0)),
            Alignment::Center,
            move || {
                icon(phosphor::MAGNIFYING_GLASS, 14.0, scheme.on_surface_variant);
                let menu = TextMenu::new(&state);
                menu.around(
                    Modifier::new().weight(1.0),
                    BoxAlignment::CenterStart,
                    move || {
                        if empty {
                            Label::new(placeholder.clone())
                                .variant(LabelVariant::Subtle)
                                .show();
                        }
                        LOCAL_TEXT_STYLE.provide(Label::new("").style(), || {
                            basic_text_field(Modifier::new().fill_max_width(), &state);
                        });
                    },
                );
            },
        );
        text.trim().to_lowercase()
    }
}
