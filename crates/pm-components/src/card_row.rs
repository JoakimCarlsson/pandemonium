//! A row of a list inside a card: a glyph, a title with badges beside it, a
//! line of detail under it, and controls at its end.

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::badge::{Badge, BadgeVariant};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a card row reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardRowVariant {
    /// An ordinary row.
    #[default]
    Plain,
    /// A row whose words read faint, for something turned off.
    Dimmed,
}

/// A card row with a variant and a size.
pub struct CardRow {
    /// The title.
    title: String,
    /// The line under it.
    detail: String,
    /// The glyph before it.
    icon: Option<Icon>,
    /// Badges after the title, each with its variant.
    badges: Vec<(String, BadgeVariant)>,
    /// Whether the title is set in the monospace face.
    code: bool,
    /// How it reads.
    variant: CardRowVariant,
    /// Its size.
    size: Size,
    /// The controls at its end.
    trailing: Option<Box<dyn FnOnce()>>,
}

impl CardRow {
    /// A row titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: String::new(),
            icon: None,
            badges: Vec::new(),
            code: false,
            variant: CardRowVariant::Plain,
            size: Size::Sm,
            trailing: None,
        }
    }

    /// The row with `detail` under its title.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// The row leading with `icon`.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The row with a badge reading `label` in `variant` after its title.
    pub fn badge(mut self, label: impl Into<String>, variant: BadgeVariant) -> Self {
        self.badges.push((label.into(), variant));
        self
    }

    /// The row with its title set in the monospace face.
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// The row in `variant`.
    pub fn variant(mut self, variant: CardRowVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The row at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The row with `trailing` at its end.
    pub fn trailing(mut self, trailing: impl FnOnce() + 'static) -> Self {
        self.trailing = Some(Box::new(trailing));
        self
    }

    /// Emits the row.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let padding = match self.size {
            Size::Xs => 4.0,
            Size::Sm => 8.0,
            Size::Md => 10.0,
            Size::Lg => 12.0,
        };
        let padding = crate::spacing(padding);
        let dim = self.variant == CardRowVariant::Dimmed;
        let tint = if dim {
            extended.subtle
        } else {
            scheme.on_surface_variant
        };
        let title_variant = match (self.code, dim) {
            (true, _) => LabelVariant::Code,
            (false, true) => LabelVariant::Muted,
            (false, false) => LabelVariant::Body,
        };
        let title = self.title;
        let detail = self.detail;
        let glyph = self.icon;
        let badges = self.badges;
        let trailing = self.trailing;
        row_arranged(
            Modifier::new().fill_max_width().padding_xy(4.0, padding),
            Arrangement::SpacedBy(crate::spacing(12.0)),
            Alignment::Center,
            move || {
                if let Some(glyph) = glyph {
                    icon(glyph, 16.0, tint);
                }
                column_arranged(
                    Modifier::new().weight(1.0),
                    Arrangement::SpacedBy(crate::spacing(2.0)),
                    Alignment::Start,
                    move || {
                        row_arranged(
                            Modifier::new(),
                            Arrangement::SpacedBy(crate::spacing(6.0)),
                            Alignment::Center,
                            move || {
                                Label::new(title).variant(title_variant).show();
                                for (label, variant) in badges {
                                    Badge::new(label).variant(variant).show();
                                }
                            },
                        );
                        if !detail.is_empty() {
                            Label::new(detail)
                                .variant(LabelVariant::Muted)
                                .size(Size::Xs)
                                .wrap()
                                .show();
                        }
                    },
                );
                if let Some(trailing) = trailing {
                    row_arranged(
                        Modifier::new(),
                        Arrangement::SpacedBy(crate::spacing(6.0)),
                        Alignment::Center,
                        trailing,
                    );
                }
            },
        );
    }
}
