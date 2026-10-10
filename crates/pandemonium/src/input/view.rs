//! The box an input is drawn as: a border, the text, and what the pointer does.
//!
//! Every box of text in the window is drawn here, so they are all the same
//! box: the same border, lit the same way when it has the keyboard, the same
//! caret, the same menu under the right button. A screen says how tall the
//! box is and what its gestures mean; everything else about it is here.

use pm_text::Position;
use pm_ui::{
    Div, Font, IntoElement, MenuItem, ResizeEvent, ResizePhase, Styled, TextSize, Theme, h_flex,
    menu_entry, menu_separator,
};

use crate::keymap::Action;

use crate::editor::{BufferView, OpenFile, plain_view};
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
/// belong to the screen, because only it knows which box this is. `solid`
/// says whether the focused caret is in its visible blink phase.
pub fn input_view<M: Clone + 'static>(
    theme: &Theme,
    input: &Input,
    focused: bool,
    solid: bool,
    lines: f32,
    on_point: impl Fn(ResizePhase, Position, Position) -> M + 'static,
    on_menu: M,
) -> Div<M> {
    text_view(
        theme,
        input.text(),
        focused,
        solid,
        lines,
        on_point,
        on_menu,
    )
}

/// Builds the box the buffer `text` is written in, for a screen that holds
/// the buffer rather than the whole input; it is drawn exactly as
/// [`input_view`] draws one.
pub fn text_view<M: Clone + 'static>(
    theme: &Theme,
    text: OpenFile,
    focused: bool,
    solid: bool,
    lines: f32,
    on_point: impl Fn(ResizePhase, Position, Position) -> M + 'static,
    on_menu: M,
) -> Div<M> {
    boxed(
        theme,
        lines,
        plain_view(text, focused)
            .caret(focused && solid)
            .on_select(on_point)
            .on_menu(on_menu),
    )
}

/// Builds the box `input` is written in, one line tall, with `placeholder`
/// standing in it while it is empty; it is drawn exactly as [`input_view`]
/// draws one.
pub fn hinted_input_view<M: Clone + 'static>(
    theme: &Theme,
    input: &Input,
    focused: bool,
    solid: bool,
    placeholder: &str,
    on_point: impl Fn(ResizePhase, Position, Position) -> M + 'static,
    on_menu: M,
) -> Div<M> {
    boxed(
        theme,
        1.0,
        hinted_view(input, focused, solid, placeholder, on_point, on_menu),
    )
}

/// Builds a single-line picker input at the theme's small monospaced text size.
pub fn compact_hinted_input_view<M: Clone + 'static>(
    theme: &Theme,
    input: &Input,
    focused: bool,
    solid: bool,
    placeholder: &str,
    on_point: impl Fn(ResizePhase, Position, Position) -> M + 'static,
    on_menu: M,
) -> Div<M> {
    boxed(
        theme,
        1.0,
        hinted_view(input, focused, solid, placeholder, on_point, on_menu)
            .font(Font::new(TextSize::Sm).mono()),
    )
}

/// Builds the editable text and placeholder shared by single-line input styles.
fn hinted_view<M: Clone + 'static>(
    input: &Input,
    focused: bool,
    solid: bool,
    placeholder: &str,
    on_point: impl Fn(ResizePhase, Position, Position) -> M + 'static,
    on_menu: M,
) -> BufferView<M> {
    plain_view(input.text(), focused)
        .caret(focused && solid)
        .placeholder(placeholder)
        .on_select(on_point)
        .on_menu(on_menu)
}

/// The border every box of text is drawn in, `lines` tall, round `view`.
fn boxed<M: Clone + 'static>(
    theme: &Theme,
    lines: f32,
    view: impl IntoElement<M> + 'static,
) -> Div<M> {
    h_flex()
        .w_full()
        .h_px(theme.size.control * lines)
        .px(1)
        .py(0.5)
        .overflow_hidden()
        .rounded(theme.radius.md)
        .bg(theme.colors.background)
        .border_1(theme.colors.border)
        .child(view)
}

/// Builds `input` with no box of its own, for a card that draws the box
/// around it and the controls beneath it, and that says how tall it is;
/// `placeholder` stands in the empty box to say what it is for, and
/// `on_scroll` is sent as its scrollbar is dragged, with the rows one
/// pixel of the drag is worth.
pub fn bare_input_view<M: Clone + 'static>(
    input: &Input,
    focused: bool,
    solid: bool,
    placeholder: &str,
    on_point: impl Fn(ResizePhase, Position, Position) -> M + 'static,
    on_scroll: impl Fn(ResizeEvent, f32) -> M + 'static,
    on_menu: M,
) -> Div<M> {
    h_flex().w_full().overflow_hidden().child(
        plain_view(input.text(), focused)
            .caret(focused && solid)
            .placeholder(placeholder)
            .rail()
            .on_select(on_point)
            .on_scroll(move |_, event, step| on_scroll(event, step))
            .on_menu(on_menu),
    )
}
