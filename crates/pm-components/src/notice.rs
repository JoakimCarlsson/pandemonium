//! A notice set into the conversation: a backup model taking over, an answer
//! to a side question, a handover or a failure.

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// The visual treatment of a notice, which picks its colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoticeVariant {
    /// Neutral information.
    #[default]
    Info,
    /// Something the reader should know went differently, such as a backup model.
    Warning,
    /// A failure.
    Danger,
    /// Something the reader asked for, such as an answer on the side.
    Accent,
}

/// A notice with a glyph, a title and a line of detail.
#[derive(Clone, Debug)]
pub struct Notice {
    /// Its first line.
    title: String,
    /// What it says beyond its title.
    detail: String,
    /// Its glyph.
    glyph: Icon,
    /// Its visual treatment.
    variant: NoticeVariant,
    /// Its text size.
    size: Size,
}

impl Notice {
    /// A notice titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: String::new(),
            glyph: phosphor::INFO,
            variant: NoticeVariant::Info,
            size: Size::Xs,
        }
    }

    /// The notice saying `detail` under its title.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// The notice with `glyph`.
    pub fn icon(mut self, glyph: Icon) -> Self {
        self.glyph = glyph;
        self
    }

    /// The notice with `variant`.
    pub fn variant(mut self, variant: NoticeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The notice at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the notice.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let tint = match self.variant {
            NoticeVariant::Info => scheme.on_surface_variant,
            NoticeVariant::Warning => extended.warning,
            NoticeVariant::Danger => scheme.error,
            NoticeVariant::Accent => scheme.primary,
        };
        row_arranged(
            Modifier::new()
                .fill_max_width()
                .background_rounded(faded(tint, 20), 6.0)
                .border(1.0, faded(tint, 70), 6.0)
                .padding_xy(10.0, 7.0),
            Arrangement::SpacedBy(8.0),
            Alignment::Start,
            move || {
                icon(self.glyph.clone(), 14.0, tint);
                column_arranged(
                    Modifier::new().weight(1.0),
                    Arrangement::SpacedBy(2.0),
                    Alignment::Start,
                    || {
                        Label::new(self.title.clone()).size(self.size).wrap().show();
                        if !self.detail.is_empty() {
                            Label::new(self.detail.clone())
                                .variant(LabelVariant::Muted)
                                .size(self.size)
                                .wrap()
                                .show();
                        }
                    },
                );
            },
        );
    }
}

/// `colour` with its opacity set to `alpha`.
pub(crate) fn faded(colour: Color, alpha: u8) -> Color {
    Color::rgba(colour.r, colour.g, colour.b, alpha)
}
