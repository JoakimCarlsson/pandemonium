//! The menu of a message, opened from the button beside it: copying it and
//! the conversation commands that act from it, such as retrying or
//! rewinding to it; the same entries open on a right click over the message.

use std::rc::Rc;

use rustle::prelude::*;

use crate::icon_button::IconButton;
use crate::menu::Menu;
use crate::size::Size;

/// The visual treatment of the menu's button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageMenuVariant {
    /// Three dots.
    #[default]
    Dots,
    /// A caret.
    Caret,
}

/// One entry of the menu.
#[derive(Clone)]
struct Entry {
    /// What it says.
    label: String,
    /// What choosing it does.
    on_click: Rc<dyn Fn()>,
}

/// A button opening a menu of entries.
#[derive(Clone, Default)]
pub struct MessageMenu {
    /// The entries, in order.
    entries: Vec<Entry>,
    /// Its visual treatment.
    variant: MessageMenuVariant,
    /// Its size.
    size: Size,
}

impl MessageMenu {
    /// An empty menu.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            variant: MessageMenuVariant::Dots,
            size: Size::Xs,
        }
    }

    /// The menu with an entry `label` that runs `on_click`.
    pub fn item(mut self, label: impl Into<String>, on_click: impl Fn() + 'static) -> Self {
        self.entries.push(Entry {
            label: label.into(),
            on_click: Rc::new(on_click),
        });
        self
    }

    /// The menu with an entry `label` that copies `text` to the clipboard.
    pub fn copy(self, label: impl Into<String>, text: String) -> Self {
        self.item(label, move || {
            if let Some(clipboard) = rustle::clipboard() {
                clipboard.set_text(&text);
            }
        })
    }

    /// The menu with `variant`.
    pub fn variant(mut self, variant: MessageMenuVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The menu at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits `content` in a column whose right click opens the entries as a menu where the pointer is.
    #[track_caller]
    pub fn around(self, content: impl FnOnce()) {
        let at = remember(|| None::<(f32, f32)>);
        let open = at.clone();
        column_with(
            Modifier::new()
                .fill_max_width()
                .on_secondary_click(move |event| {
                    open.set(Some((event.position.x, event.position.y)))
                }),
            move || {
                if let Some((across, down)) = at.get() {
                    let dismiss = at.clone();
                    let mut menu = Menu::new()
                        .offset(across, down)
                        .width(240.0)
                        .on_dismiss(move || dismiss.set(None));
                    for entry in self.entries {
                        let close = at.clone();
                        let on_click = entry.on_click.clone();
                        menu = menu.item(entry.label, move || {
                            close.set(None);
                            on_click();
                        });
                    }
                    menu.show();
                }
                content();
            },
        );
    }

    /// Emits the button and, while open, the menu under it.
    #[track_caller]
    pub fn show(self) {
        let open = remember(|| false);
        let toggle = open.clone();
        let dismiss = open.clone();
        column_with(Modifier::new(), move || {
            IconButton::new(match self.variant {
                MessageMenuVariant::Dots => phosphor::DOTS_THREE,
                MessageMenuVariant::Caret => phosphor::CARET_DOWN,
            })
            .size(self.size)
            .active(open.get())
            .on_click(move || toggle.set(!toggle.get()))
            .show();
            let close = open.clone();
            dropdown_menu(
                open.get(),
                move || dismiss.set(false),
                move || {
                    for entry in &self.entries {
                        let on_click = entry.on_click.clone();
                        let close = close.clone();
                        dropdown_menu_item(&entry.label, move || {
                            close.set(false);
                            on_click();
                        });
                    }
                },
            );
        });
    }
}
