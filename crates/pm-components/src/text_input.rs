//! A box one line of text is typed in.

use std::cell::RefCell;
use std::rc::Rc;

use rustle::prelude::*;
use crate::text_menu::TextMenu;
use rustle::{TextFieldOptions, basic_text_field_in, remember_focus};

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a text box is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextInputVariant {
    /// The window background with a hairline border.
    #[default]
    Outlined,
    /// A raised surface with a magnifier before the text, for searching.
    Search,
}

/// A text box with a variant and a size, which follows the value it is
/// given and tells what is typed.
#[derive(Clone)]
pub struct TextInput {
    /// The text it holds, as the model has it.
    value: String,
    /// What it says while empty.
    placeholder: String,
    /// How it is drawn.
    variant: TextInputVariant,
    /// Its size.
    size: Size,
    /// Whether the text is set in the monospace face.
    code: bool,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What typing does, given the text.
    on_change: Option<Rc<dyn Fn(String)>>,
    /// What Enter does, given the text.
    on_submit: Option<Rc<dyn Fn(String)>>,
    /// Whether it takes the keyboard when it first shows.
    autofocus: bool,
}

impl TextInput {
    /// A box holding `value`.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            placeholder: String::new(),
            variant: TextInputVariant::Outlined,
            size: Size::Sm,
            code: false,
            modifier: Modifier::new(),
            on_change: None,
            on_submit: None,
            autofocus: false,
        }
    }

    /// The box taking the keyboard when it first shows.
    pub fn autofocus(mut self) -> Self {
        self.autofocus = true;
        self
    }

    /// The box saying `placeholder` while empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The box in `variant`.
    pub fn variant(mut self, variant: TextInputVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The box at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The box set in the monospace face.
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// The box laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The box calling `on_change` with its text whenever typing changes it.
    pub fn on_change(mut self, on_change: impl Fn(String) + 'static) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }

    /// The box calling `on_submit` with its text when Enter is pressed in it.
    pub fn on_submit(mut self, on_submit: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }

    /// Emits the box.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let state = remember_text_field_state(&self.value);
        let focus = remember_focus();
        let focused_once = remember(|| false);
        if self.autofocus && !focused_once.get() {
            focused_once.set(true);
            focus.request_focus();
        }
        let given: Rc<RefCell<String>> =
            remember(|| Rc::new(RefCell::new(self.value.clone()))).get();
        let reported: Rc<RefCell<String>> =
            remember(|| Rc::new(RefCell::new(self.value.clone()))).get();
        if *given.borrow() != self.value {
            *given.borrow_mut() = self.value.clone();
            *reported.borrow_mut() = self.value.clone();
            if state.text() != self.value {
                state.set_text(self.value.clone());
            }
        }
        let text = state.text();
        if *reported.borrow() != text {
            *reported.borrow_mut() = text.clone();
            if let Some(on_change) = self.on_change.clone() {
                side_effect(move || on_change(text));
            }
        }
        if let Some(on_submit) = self.on_submit.clone() {
            let state = state.clone();
            state.clone().on_submit(move || on_submit(state.text()));
        }
        let height = match self.size {
            Size::Xs => 22.0,
            Size::Sm => 28.0,
            Size::Md => 32.0,
            Size::Lg => 38.0,
        };
        let height = crate::row_height(height);
        let fill = match self.variant {
            TextInputVariant::Outlined => scheme.background,
            TextInputVariant::Search => scheme.surface_container,
        };
        let empty = state.text().is_empty();
        let placeholder = self.placeholder;
        let variant = self.variant;
        let mut style = Label::new("").size(self.size).style();
        if self.code {
            style.font_family = crate::code_font().family;
        }
        row_arranged(
            self.modifier
                .height(height)
                .background_rounded(fill, 6.0)
                .border(1.0, scheme.outline_variant, 6.0)
                .padding_xy(10.0, 0.0),
            Arrangement::SpacedBy(crate::spacing(8.0)),
            Alignment::Center,
            move || {
                if variant == TextInputVariant::Search {
                    icon(phosphor::MAGNIFYING_GLASS, 14.0, scheme.on_surface_variant);
                }
                TextMenu::new(&state).around(
                    Modifier::new().weight(1.0).clip(),
                    BoxAlignment::CenterStart,
                    || {
                        if empty {
                            Label::new(placeholder).variant(LabelVariant::Subtle).show();
                        }
                        LOCAL_TEXT_STYLE.provide(style, || {
                            basic_text_field_in(
                                Modifier::new().fill_max_width(),
                                Modifier::new(),
                                &state,
                                &focus,
                                TextFieldOptions::default(),
                            );
                        });
                    },
                );
            },
        );
    }
}
