//! The heading of a page or of a group on it: an optional trail above the
//! title, the title, and what the page is for under it.

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;

/// How a section header reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SectionHeaderVariant {
    /// The large title of a page.
    #[default]
    Page,
    /// The muted, small title of a group of rows.
    Group,
}

/// A section header with a variant and a size.
#[derive(Clone)]
pub struct SectionHeader {
    /// The title.
    title: String,
    /// The page it sits inside, shown above it.
    trail: Option<String>,
    /// What the page is for.
    summary: String,
    /// How it reads.
    variant: SectionHeaderVariant,
    /// Its size.
    size: Size,
}

impl SectionHeader {
    /// A page header titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            trail: None,
            summary: String::new(),
            variant: SectionHeaderVariant::Page,
            size: Size::Lg,
        }
    }

    /// The header with `trail` above the title, when there is one.
    pub fn trail(mut self, trail: Option<String>) -> Self {
        self.trail = trail;
        self
    }

    /// The header saying `summary` under the title.
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    /// The header in `variant`.
    pub fn variant(mut self, variant: SectionHeaderVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The header at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the header.
    #[track_caller]
    pub fn show(self) {
        let title = self.title;
        let trail = self.trail;
        let summary = self.summary;
        let variant = self.variant;
        let size = self.size;
        column_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::SpacedBy(4.0),
            Alignment::Start,
            move || {
                if let Some(trail) = trail {
                    Label::new(trail)
                        .variant(LabelVariant::Muted)
                        .size(Size::Xs)
                        .show();
                }
                match variant {
                    SectionHeaderVariant::Page => Label::new(title)
                        .variant(LabelVariant::Heading)
                        .size(if size == Size::Lg { Size::Md } else { size })
                        .show(),
                    SectionHeaderVariant::Group => Label::new(title)
                        .variant(LabelVariant::Muted)
                        .size(Size::Xs)
                        .show(),
                }
                if !summary.is_empty() {
                    Label::new(summary)
                        .variant(LabelVariant::Muted)
                        .wrap()
                        .modifier(Modifier::new().width_in(0.0, 640.0))
                        .show();
                }
            },
        );
    }
}
