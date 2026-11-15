//! The menu of a text field: Cut, Copy, Paste and Select all, opened by a
//! right click where the pointer is, for every field that takes text.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Focus, TextFieldState, clipboard};

use crate::menu::Menu;
use crate::size::Size;

/// What pasting does before the text is pasted, saying whether it took the clipboard itself, as the composer does with files and pictures.
type PasteHook = Rc<dyn Fn() -> bool>;

/// The menu of a text field, with a size and the side it opens on.
#[derive(Clone)]
pub struct TextMenu {
    /// The text field it acts on.
    state: TextFieldState,
    /// Whether it opens upward, for a field near the bottom of the window.
    above: bool,
    /// How tall its rows are.
    size: Size,
    /// What pasting does first, when anything.
    on_paste: Option<PasteHook>,
    /// The field's focus, given back to it after each command so what it selected shows and stays.
    focus: Option<Focus>,
}

impl TextMenu {
    /// The menu of the field `state` holds, opening downward.
    pub fn new(state: &TextFieldState) -> Self {
        Self {
            state: state.clone(),
            above: false,
            size: Size::Xs,
            on_paste: None,
            focus: None,
        }
    }

    /// The menu giving the keyboard back to the field whose focus is `focus` after each command.
    pub fn focus(mut self, focus: &Focus) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    /// The menu opening upward when `above`.
    pub fn above(mut self, above: bool) -> Self {
        self.above = above;
        self
    }

    /// The menu with rows at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The menu calling `on_paste` before pasting text, which pastes nothing when it says it took the clipboard.
    pub fn on_paste(mut self, on_paste: impl Fn() -> bool + 'static) -> Self {
        self.on_paste = Some(Rc::new(on_paste));
        self
    }

    /// Emits `content` in a stack laid out with `modifier` and `alignment`, whose right click opens the menu where the pointer is.
    #[track_caller]
    pub fn around(self, modifier: Modifier, alignment: BoxAlignment, content: impl FnOnce()) {
        let at = remember(|| None::<(f32, f32)>);
        let opener = at.clone();
        stack_aligned(
            modifier.on_context_menu(move |event| {
                opener.set(Some((event.position.x, event.position.y)))
            }),
            alignment,
            move || {
                content();
                if let Some(point) = at.get() {
                    let closer = at.clone();
                    self.show_at(point, move || closer.set(None));
                }
            },
        );
    }

    /// Emits the menu at `at` within what holds it, across and down, calling `on_dismiss` when it closes.
    #[track_caller]
    pub fn show_at(self, at: (f32, f32), on_dismiss: impl Fn() + 'static) {
        let selected = self
            .state
            .value()
            .selected_text()
            .map(str::to_owned)
            .unwrap_or_default();
        let cut_state = self.state.clone();
        let cut_text = selected.clone();
        let paste_state = self.state.clone();
        let all_state = self.state.clone();
        let on_paste = self.on_paste.clone();
        let size = self.size;
        let above = self.above;
        let refocus = self.focus.clone();
        let back = move || {
            if let Some(focus) = &refocus {
                focus.request_focus();
            }
        };
        let (cut_back, paste_back, all_back) = (back.clone(), back.clone(), back);
        stack_with(Modifier::new().offset(at.0, at.1), move || {
            let mut menu = Menu::new().size(size).width(180.0).above(above);
            menu = if selected.is_empty() {
                menu.disabled_item(phosphor::SCISSORS, "Cut")
                    .disabled_item(phosphor::COPY, "Copy")
            } else {
                menu.icon_item(phosphor::SCISSORS, "Cut", move || {
                    copy_text(&cut_text);
                    cut_state.replace_selection("");
                    cut_back();
                })
                .hint("Ctrl+X")
                .icon_item(phosphor::COPY, "Copy", move || copy_text(&selected))
                .hint("Ctrl+C")
            };
            menu.icon_item(phosphor::CLIPBOARD_TEXT, "Paste", move || {
                if !on_paste.as_ref().is_some_and(|hook| hook())
                    && let Some(text) = clipboard().and_then(|clipboard| clipboard.text())
                {
                    paste_state.replace_selection(&text);
                }
                paste_back();
            })
            .hint("Ctrl+V")
            .separator()
            .icon_item(phosphor::SELECTION_ALL, "Select all", move || {
                all_state.select_all();
                all_back();
            })
            .hint("Ctrl+A")
            .on_dismiss(on_dismiss)
            .show();
        });
    }
}

/// Puts `text` on the clipboard.
pub(crate) fn copy_text(text: &str) {
    if let Some(clipboard) = clipboard() {
        clipboard.set_text(text);
    }
}
