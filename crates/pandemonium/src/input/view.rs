//! The box an input is drawn as: a border, the text, and what the pointer does.
//!
//! Every box of text in the window is drawn here, so they are all the same
//! box: the same border, lit the same way when it has the keyboard, the same
//! caret, the same menu under the right button. A screen says how tall the
//! box is and what its gestures mean; everything else about it is here.

use pm_text::Position;
use pm_ui::{Div, MenuItem, ResizePhase, Styled, Theme, h_flex, menu_entry, menu_separator};

use crate::keymap::Action;

use crate::editor::plain_view;
use crate::input::state::Input;
use crate::message::Message;

/// The things that can be done to the text in a box.
///
/// Every entry is the command a keybinding would ask for, carried out in
/// whichever box has the keyboard — so the menu is a second way of asking
/// rather than a second set of commands. Cutting and copying nothing is
/// greyed rather than left out, so the menu keeps its shape.
pub fn input_menu(selected: bool) -> Vec<MenuItem<Message>> {
    let selected = |action: Action| selected.then_some(Message::EditText(action));

    vec![
        menu_entry("Cut", selected(Action::Cut)),
        menu_entry("Copy", selected(Action::Copy)),
        menu_entry("Paste", Some(Message::EditText(Action::Paste))),
        menu_separator(),
        menu_entry("Select All", Some(Message::EditText(Action::SelectAll))),
        menu_entry("Undo", Some(Message::EditText(Action::Undo))),
        menu_entry("Redo", Some(Message::EditText(Action::Redo))),
    ]
}

/// Builds the box `input` is written in, `lines` of its own text tall.
///
/// `on_point` is sent as the pointer presses, drags and is let go of in the
/// text, and `on_menu` when the right button asks for a menu over it — both
/// belong to the screen, because only it knows which box this is.
pub fn input_view<M: Clone + 'static>(
    theme: &Theme,
    input: &Input,
    focused: bool,
    lines: f32,
    on_point: impl Fn(ResizePhase, Position, Position) -> M + 'static,
    on_menu: M,
) -> Div<M> {
    h_flex()
        .w_full()
        .h_px(theme.size.control * lines)
        .px(1)
        .py(0.5)
        .overflow_hidden()
        .rounded(theme.radius.md)
        .bg(theme.colors.background)
        .border_1(match focused {
            true => theme.colors.border_focused,
            false => theme.colors.border,
        })
        .child(
            plain_view(input.text(), focused)
                .caret(focused)
                .on_select(on_point)
                .on_menu(on_menu),
        )
}
