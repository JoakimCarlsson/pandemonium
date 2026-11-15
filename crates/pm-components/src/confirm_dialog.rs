//! A question asked before something that cannot be taken back, over a
//! dimmed layer covering the area it is emitted in: a title, what it will
//! do, an optional box to name something, and Cancel and confirm buttons.

use std::rc::Rc;

use rustle::prelude::*;

use crate::button::{Button, ButtonVariant};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::text_input::TextInput;
use rustle::color_scheme;
use rustle::{Key, Modifiers};

/// How a confirmation reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConfirmDialogVariant {
    /// The confirm button in the primary style.
    #[default]
    Plain,
    /// The confirm button as a warning, for throwing work away.
    Destructive,
}

/// A box the question asks a name in.
#[derive(Clone)]
struct Naming {
    /// The name typed so far.
    value: String,
    /// What the box says while empty.
    placeholder: String,
    /// What typing does, given the text.
    on_change: Rc<dyn Fn(String)>,
}

/// A confirmation with a variant and a size.
#[derive(Clone)]
pub struct ConfirmDialog {
    /// Its title.
    title: String,
    /// What it will do.
    message: String,
    /// What the confirm button reads.
    confirm: String,
    /// The box it asks a name in, when it asks one.
    naming: Option<Naming>,
    /// How it reads.
    variant: ConfirmDialogVariant,
    /// Its size.
    size: Size,
    /// What confirming does.
    on_confirm: Option<Rc<dyn Fn()>>,
    /// What cancelling, or a click outside, does.
    on_cancel: Option<Rc<dyn Fn()>>,
}

impl ConfirmDialog {
    /// A question titled `title` saying `message`, confirmed by a button reading `confirm`.
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        confirm: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            confirm: confirm.into(),
            naming: None,
            variant: ConfirmDialogVariant::Plain,
            size: Size::Sm,
            on_confirm: None,
            on_cancel: None,
        }
    }

    /// The question with a box holding `value`, saying `placeholder` while empty, calling `on_change` as it is typed in.
    pub fn naming(
        mut self,
        value: impl Into<String>,
        placeholder: impl Into<String>,
        on_change: impl Fn(String) + 'static,
    ) -> Self {
        self.naming = Some(Naming {
            value: value.into(),
            placeholder: placeholder.into(),
            on_change: Rc::new(on_change),
        });
        self
    }

    /// The question in `variant`.
    pub fn variant(mut self, variant: ConfirmDialogVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The question at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The question calling `on_confirm` when confirmed, or when Enter is pressed in its box.
    pub fn on_confirm(mut self, on_confirm: impl Fn() + 'static) -> Self {
        self.on_confirm = Some(Rc::new(on_confirm));
        self
    }

    /// The question calling `on_cancel` when cancelled, clicked outside, or Escape is pressed in it.
    pub fn on_cancel(mut self, on_cancel: impl Fn() + 'static) -> Self {
        self.on_cancel = Some(Rc::new(on_cancel));
        self
    }

    /// Emits the question over the whole area it is given; emit it last in a stack.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let width = match self.size {
            Size::Xs => 240.0,
            Size::Sm => 290.0,
            Size::Md => 360.0,
            Size::Lg => 440.0,
        };
        let cancel = self.on_cancel.clone();
        let escape = self.on_cancel.clone();
        let confirm = self.on_confirm.clone();
        stack_aligned(
            Modifier::new()
                .fill_max_size()
                .background(Color::rgba(0, 0, 0, 120))
                .clickable_plain(move || {
                    if let Some(cancel) = &cancel {
                        cancel();
                    }
                })
                .on_key_event(move |event| match &escape {
                    Some(escape) if event.is(Key::Escape, Modifiers::NONE) => {
                        escape();
                        true
                    }
                    _ => false,
                }),
            BoxAlignment::Center,
            move || {
                column_arranged(
                    Modifier::new()
                        .width(width)
                        .shadow(10.0, 10.0)
                        .background_rounded(scheme.surface_container, 10.0)
                        .border(1.0, scheme.outline_variant, 10.0)
                        .clickable_plain(|| {})
                        .padding(16.0),
                    Arrangement::SpacedBy(crate::spacing(10.0)),
                    Alignment::Start,
                    move || {
                        Label::new(self.title).variant(LabelVariant::Heading).show();
                        Label::new(self.message)
                            .variant(LabelVariant::Muted)
                            .size(Size::Xs)
                            .wrap()
                            .modifier(Modifier::new().fill_max_width())
                            .show();
                        if let Some(naming) = self.naming {
                            let submit = confirm.clone();
                            let change = naming.on_change.clone();
                            TextInput::new(naming.value)
                                .placeholder(naming.placeholder)
                                .size(Size::Sm)
                                .modifier(Modifier::new().fill_max_width())
                                .on_change(move |text| change(text))
                                .on_submit(move |_| {
                                    if let Some(submit) = &submit {
                                        submit();
                                    }
                                })
                                .show();
                        }
                        row_arranged(
                            Modifier::new().fill_max_width(),
                            Arrangement::SpacedBy(crate::spacing(8.0)),
                            Alignment::Center,
                            move || {
                                spacer(Modifier::new().weight(1.0));
                                let cancel = self.on_cancel.clone();
                                Button::new("Cancel")
                                    .variant(ButtonVariant::Ghost)
                                    .size(Size::Xs)
                                    .on_click(move || {
                                        if let Some(cancel) = &cancel {
                                            cancel();
                                        }
                                    })
                                    .show();
                                let confirm = confirm.clone();
                                let button = Button::new(self.confirm)
                                    .variant(match self.variant {
                                        ConfirmDialogVariant::Plain => ButtonVariant::Primary,
                                        ConfirmDialogVariant::Destructive => ButtonVariant::Danger,
                                    })
                                    .size(Size::Xs)
                                    .on_click(move || {
                                        if let Some(confirm) = &confirm {
                                            confirm();
                                        }
                                    });
                                button.show();
                            },
                        );
                    },
                );
            },
        );
    }
}
