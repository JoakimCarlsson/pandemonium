//! The bar across the top of a page: what the page is, where it lives, and
//! the page's own buttons at its end.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a page header reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageHeaderVariant {
    /// A hairline under the bar.
    #[default]
    Ruled,
    /// No line under the bar.
    Plain,
}

/// A page's title bar with a variant and a size.
#[derive(Clone)]
pub struct PageHeader {
    /// The page's title.
    title: String,
    /// Faint text after the title, such as a folder.
    subtitle: Option<String>,
    /// A composable before the title, such as an avatar.
    leading: Option<Rc<dyn Fn()>>,
    /// How it reads.
    variant: PageHeaderVariant,
    /// Its height and title size.
    size: Size,
}

impl PageHeader {
    /// A ruled header titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            leading: None,
            variant: PageHeaderVariant::Ruled,
            size: Size::Md,
        }
    }

    /// The header with faint `subtitle` after the title.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The header leading with `leading`, such as an avatar.
    pub fn leading(mut self, leading: impl Fn() + 'static) -> Self {
        self.leading = Some(Rc::new(leading));
        self
    }

    /// The header in `variant`.
    pub fn variant(mut self, variant: PageHeaderVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The header at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the header with `trailing`, such as buttons, at its end.
    pub fn show(self, trailing: impl FnOnce()) {
        let scheme = color_scheme();
        let height = match self.size {
            Size::Xs => 36.0,
            Size::Sm => 42.0,
            Size::Md => 48.0,
            Size::Lg => 56.0,
        };
        let mut modifier = Modifier::new().fill_max_width().height(height);
        if self.variant == PageHeaderVariant::Ruled {
            modifier = modifier.border_bottom_hairline(scheme.outline_variant);
        }
        row_arranged(
            modifier.padding_xy(20.0, 0.0),
            Arrangement::SpacedBy(12.0),
            Alignment::Center,
            move || {
                if let Some(leading) = &self.leading {
                    leading();
                }
                Label::new(self.title.clone())
                    .variant(LabelVariant::Heading)
                    .size(Size::Md)
                    .show();
                match &self.subtitle {
                    Some(subtitle) => Label::new(subtitle.clone())
                        .variant(LabelVariant::Subtle)
                        .size(Size::Xs)
                        .modifier(Modifier::new().weight(1.0))
                        .show(),
                    None => spacer(Modifier::new().weight(1.0)),
                }
                trailing();
            },
        );
    }
}
