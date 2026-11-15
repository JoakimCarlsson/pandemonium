//! A dialog over the window: a dimmed backdrop and a panel in the middle with a title over its content.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a dialog is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogVariant {
    /// A panel on a dimmed window, closed by pressing beside it.
    #[default]
    Modal,
    /// A panel on a dimmed window that stays until one of its own buttons closes it.
    Blocking,
}

/// A panel over the dimmed window, with a title over what it holds.
pub struct Dialog {
    /// What the title says.
    title: String,
    /// How it is drawn.
    variant: DialogVariant,
    /// How wide the panel is.
    size: Size,
    /// Called when the reader presses beside the panel.
    on_dismiss: Option<Rc<dyn Fn()>>,
}

impl Dialog {
    /// A modal dialog titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            variant: DialogVariant::Modal,
            size: Size::Md,
            on_dismiss: None,
        }
    }

    /// The dialog in `variant`.
    pub fn variant(mut self, variant: DialogVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The dialog in `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Calls `on_dismiss` when the reader presses beside a modal dialog's panel.
    pub fn on_dismiss(mut self, on_dismiss: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(on_dismiss));
        self
    }

    /// How wide the panel is.
    fn width(&self) -> f32 {
        match self.size {
            Size::Xs => 320.0,
            Size::Sm => 400.0,
            Size::Md => 520.0,
            Size::Lg => 680.0,
        }
    }

    /// Emits the dialog around `content`, laid out as a column under the title.
    #[track_caller]
    pub fn show(self, content: impl FnOnce()) {
        let scheme = color_scheme();
        let width = self.width();
        let dismiss = match self.variant {
            DialogVariant::Modal => self.on_dismiss.clone(),
            DialogVariant::Blocking => None,
        };
        let title = self.title;
        stack_aligned(
            Modifier::new()
                .fill_max_size()
                .background(Color::rgba(0, 0, 0, 140))
                .on_press(move |_| {
                    if let Some(dismiss) = &dismiss {
                        dismiss();
                    }
                }),
            BoxAlignment::Center,
            move || {
                column_arranged(
                    Modifier::new()
                        .width(width)
                        .background_rounded(scheme.surface_container, 12.0)
                        .border(1.0, scheme.outline_variant, 12.0)
                        .on_press(|_| {})
                        .padding_xy(20.0, 18.0),
                    Arrangement::SpacedBy(crate::spacing(12.0)),
                    Alignment::Start,
                    move || {
                        Label::new(title)
                            .variant(LabelVariant::Heading)
                            .size(Size::Md)
                            .show();
                        content();
                    },
                );
            },
        );
    }
}
