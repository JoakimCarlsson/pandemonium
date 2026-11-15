//! A titled group of rows in one outlined box, with hairlines between the
//! rows.

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a section is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SectionVariant {
    /// The rows in an outlined box on the raised surface.
    #[default]
    Boxed,
    /// The rows straight on the page, under a larger heading.
    Open,
}

/// A titled group of rows with a variant and a size.
#[derive(Clone)]
pub struct Section {
    /// The heading over the rows.
    title: String,
    /// How it is drawn.
    variant: SectionVariant,
    /// The size of the heading.
    size: Size,
}

impl Section {
    /// A boxed section headed `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            variant: SectionVariant::Boxed,
            size: Size::Xs,
        }
    }

    /// The section in `variant`.
    pub fn variant(mut self, variant: SectionVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The section's heading at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the heading and `count` rows, emitting row `index` with `row`;
    /// nothing at all when `count` is zero.
    pub fn show(self, count: usize, row: impl Fn(usize)) {
        if count == 0 {
            return;
        }
        let scheme = color_scheme();
        let variant = self.variant;
        column_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::SpacedBy(crate::spacing(8.0)),
            Alignment::Start,
            || {
                let heading = match variant {
                    SectionVariant::Boxed => LabelVariant::Subtle,
                    SectionVariant::Open => LabelVariant::Heading,
                };
                Label::new(self.title.clone())
                    .variant(heading)
                    .size(self.size)
                    .modifier(Modifier::new().padding_xy(
                        if variant == SectionVariant::Open {
                            12.0
                        } else {
                            0.0
                        },
                        0.0,
                    ))
                    .show();
                let modifier = match variant {
                    SectionVariant::Boxed => Modifier::new()
                        .fill_max_width()
                        .background_rounded(scheme.surface_container, 8.0)
                        .border(1.0, scheme.outline_variant, 8.0)
                        .clip_rounded(8.0),
                    SectionVariant::Open => Modifier::new().fill_max_width(),
                };
                column_with(modifier, || {
                    for index in 0..count {
                        key(index, || {
                            if index > 0 && variant == SectionVariant::Boxed {
                                spacer(
                                    Modifier::new()
                                        .fill_max_width()
                                        .height(1.0)
                                        .background(scheme.outline_variant),
                                );
                            }
                            row(index);
                        });
                    }
                });
            },
        );
    }
}
