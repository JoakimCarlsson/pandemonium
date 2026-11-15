//! Keyboard focus for controls: a control the reader can tab to, pressed by
//! Enter or Space, showing a ring while the keyboard rather than the pointer
//! put it in focus.

use rustle::prelude::*;
use rustle::{Focus, Key, KeyEvent, remember_focus};

/// The focus of one control and whether the pointer gave it.
#[derive(Clone)]
pub(crate) struct FocusRing {
    /// The control's focus.
    pub(crate) focus: Focus,
    /// Whether the control was last focused by a click, which shows no ring.
    clicked: State<bool>,
}

impl FocusRing {
    /// Whether the ring shows: the control holds the keyboard and was not clicked into it.
    pub(crate) fn shown(&self) -> bool {
        self.focus.is_focused() && !self.clicked.get()
    }

    /// Notes that the pointer pressed the control, so its focus shows no ring.
    pub(crate) fn clicked(&self) {
        if !self.clicked.get() {
            self.clicked.set(true);
        }
    }

    /// `modifier` made able to hold the keyboard, doing `press` on Enter or Space while it does.
    pub(crate) fn focusable(&self, modifier: Modifier, press: impl Fn() + 'static) -> Modifier {
        modifier.focusable(&self.focus, move |event| activates(event, &press))
    }
}

/// Remembers the focus of the control at this call site, forgetting a click once the control lost the keyboard.
#[track_caller]
pub(crate) fn remember_focus_ring() -> FocusRing {
    let focus = remember_focus();
    let clicked = remember(|| false);
    if clicked.get() && !focus.is_focused() {
        clicked.set(false);
    }
    FocusRing { focus, clicked }
}

/// Does `open` when `event` presses the down arrow, or Alt with it, as a list box opens its list, and says whether it did.
pub(crate) fn opens(event: &KeyEvent, open: &dyn Fn()) -> bool {
    let modifiers = event.modifiers;
    if event.pressed
        && event.key == Key::ArrowDown
        && !(modifiers.ctrl || modifiers.meta || modifiers.shift)
    {
        open();
        return true;
    }
    false
}

/// Does `press` when `event` presses Enter or Space with no modifier held, and says whether it did.
pub(crate) fn activates(event: &KeyEvent, press: &dyn Fn()) -> bool {
    let modifiers = event.modifiers;
    let plain = !(modifiers.ctrl || modifiers.alt || modifiers.meta || modifiers.shift);
    let pressing = match &event.key {
        Key::Enter => true,
        Key::Character(text) => text == " ",
        _ => false,
    };
    if event.pressed && plain && pressing {
        press();
        return true;
    }
    false
}
