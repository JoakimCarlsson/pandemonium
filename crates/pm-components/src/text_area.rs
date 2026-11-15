//! A box several lines of text are typed in, such as a commit message,
//! which follows the value it is given, tells what is typed and submits on
//! Ctrl+Enter.

use crate::text_menu::TextMenu;
use std::cell::RefCell;
use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, TextFieldOptions, basic_text_field_in, remember_focus};

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a text area is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAreaVariant {
    /// The window background with a hairline border.
    #[default]
    Outlined,
    /// A sunken surface with no border.
    Sunken,
}

/// A text area with a variant and a size.
#[derive(Clone)]
pub struct TextArea {
    /// The text it holds, as the model has it.
    value: String,
    /// What it says while empty.
    placeholder: String,
    /// How it is drawn.
    variant: TextAreaVariant,
    /// Its text size.
    size: Size,
    /// How tall it is.
    height: f32,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What typing does, given the text.
    on_change: Option<Rc<dyn Fn(String)>>,
    /// What Ctrl+Enter does, given the text.
    on_submit: Option<Rc<dyn Fn(String)>>,
}

impl TextArea {
    /// An area holding `value`.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            placeholder: String::new(),
            variant: TextAreaVariant::Outlined,
            size: Size::Sm,
            height: 64.0,
            modifier: Modifier::new(),
            on_change: None,
            on_submit: None,
        }
    }

    /// The area saying `placeholder` while empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The area in `variant`.
    pub fn variant(mut self, variant: TextAreaVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The area with text at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The area `height` tall.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The area laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The area calling `on_change` with the text whenever it changes.
    pub fn on_change(mut self, on_change: impl Fn(String) + 'static) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }

    /// The area calling `on_submit` with the text on Ctrl+Enter.
    pub fn on_submit(mut self, on_submit: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }

    /// Emits the area.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let state = remember_text_field_state(&self.value);
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
        let submit_state = state.clone();
        let on_submit = self.on_submit.clone();
        let mut modifier = self
            .modifier
            .height(self.height)
            .on_preview_key_event(move |event| {
                let submit = event.pressed && event.key == Key::Enter && event.modifiers.ctrl;
                if submit && let Some(on_submit) = &on_submit {
                    on_submit(submit_state.text());
                }
                submit
            })
            .padding_xy(8.0, 6.0);
        modifier = match self.variant {
            TextAreaVariant::Outlined => modifier
                .background_rounded(scheme.background, 6.0)
                .border(1.0, scheme.outline_variant, 6.0),
            TextAreaVariant::Sunken => modifier.background_rounded(scheme.surface_container, 6.0),
        };
        let empty = state.text().is_empty();
        let placeholder = self.placeholder;
        let style = Label::new("").size(self.size).wrap().style();
        let size = self.size;
        let focus = remember_focus();
        let menu = TextMenu::new(&state).focus(&focus);
        menu.around(modifier.clip(), BoxAlignment::TopStart, move || {
            if empty {
                Label::new(placeholder)
                    .variant(LabelVariant::Subtle)
                    .size(size)
                    .show();
            }
            LOCAL_TEXT_STYLE.provide(style, || {
                basic_text_field_in(
                    Modifier::new().fill_max_size().clip(),
                    Modifier::new(),
                    &state,
                    &focus,
                    TextFieldOptions {
                        multiline: true,
                        ..TextFieldOptions::default()
                    },
                );
            });
        });
    }
}
