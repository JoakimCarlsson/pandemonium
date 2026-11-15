//! A modal: a dimmed layer over the whole window with a surface on it,
//! centred or hung near the top, closed by a click on the dimmed layer or
//! Escape. Emit it last in a stack that fills the window.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, Modifiers};

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// Where a modal's surface sits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModalVariant {
    /// In the middle of the window, for a question or a form.
    #[default]
    Centred,
    /// Near the top, under the command centre, for the quick input.
    Top,
}

/// A modal with a variant and a size.
#[derive(Clone)]
pub struct Modal {
    /// The heading over its content.
    title: Option<String>,
    /// The text under the heading.
    message: Option<String>,
    /// How wide the surface is, by default from its size.
    width: Option<f32>,
    /// Where it sits.
    variant: ModalVariant,
    /// Its size, which sets its width and padding.
    size: Size,
}

impl Modal {
    /// A centred modal with no heading.
    pub fn new() -> Self {
        Self {
            title: None,
            message: None,
            width: None,
            variant: ModalVariant::Centred,
            size: Size::Md,
        }
    }

    /// The modal headed `title`.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The modal saying `message` under its heading.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// The modal's surface `width` wide.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The modal in `variant`.
    pub fn variant(mut self, variant: ModalVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The modal at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the modal holding `content`, calling `on_dismiss` on a click on the dimmed layer or Escape.
    #[track_caller]
    pub fn show(self, on_dismiss: impl Fn() + 'static, content: impl FnOnce()) {
        let scheme = color_scheme();
        let on_dismiss: Rc<dyn Fn()> = Rc::new(on_dismiss);
        let escape = on_dismiss.clone();
        let (width, padding) = match self.size {
            Size::Xs => (280.0, 12.0),
            Size::Sm => (360.0, 16.0),
            Size::Md => (440.0, 20.0),
            Size::Lg => (600.0, 0.0),
        };
        let width = self.width.unwrap_or(width);
        let (alignment, top) = match self.variant {
            ModalVariant::Centred => (BoxAlignment::Center, 0.0),
            ModalVariant::Top => (BoxAlignment::TopCenter, 56.0),
        };
        let title = self.title;
        let message = self.message;
        stack_aligned(
            Modifier::new()
                .fill_max_size()
                .z_index_unclipped(2)
                .background(Color::rgba(0, 0, 0, 110))
                .clickable_plain(move || on_dismiss())
                .on_key_event(move |event| {
                    if event.is(Key::Escape, Modifiers::NONE) {
                        escape();
                        return true;
                    }
                    false
                }),
            alignment,
            move || {
                column_arranged(
                    Modifier::new()
                        .offset(0.0, top)
                        .width(width)
                        .shadow(16.0, 10.0)
                        .background_rounded(scheme.surface_container, 10.0)
                        .border(1.0, scheme.outline_variant, 10.0)
                        .clickable_plain(|| {})
                        .padding(padding),
                    Arrangement::SpacedBy(crate::spacing(8.0)),
                    Alignment::Start,
                    move || {
                        if let Some(title) = title {
                            Label::new(title)
                                .variant(LabelVariant::Heading)
                                .size(Size::Md)
                                .show();
                        }
                        if let Some(message) = message {
                            Label::new(message)
                                .variant(LabelVariant::Muted)
                                .wrap()
                                .modifier(Modifier::new().fill_max_width())
                                .show();
                        }
                        content();
                    },
                );
            },
        );
    }
}

impl Default for Modal {
    /// A centred modal with no heading.
    fn default() -> Self {
        Self::new()
    }
}
