//! The keys the state logic matches on, and how a key event from the window becomes one.
//!
//! The host owns the one key handler of the window: it turns each
//! [`rustle::KeyEvent`] into a [`KeyEvent`] with [`KeyEvent::from_window`] and
//! hands it to the same logic that matched on keys before, which decides by
//! the focused [`FocusId`](crate::kit::FocusId) what the key does.

/// A key the app reacts to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    /// A key that types a character, as the keyboard layout and Shift make it.
    Character(char),
    /// Enter or Return.
    Enter,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// The space bar.
    Space,
    /// The up arrow.
    ArrowUp,
    /// The down arrow.
    ArrowDown,
    /// The left arrow.
    ArrowLeft,
    /// The right arrow.
    ArrowRight,
    /// A function key, `Function(1)` for F1.
    Function(u8),
}

/// Which modifier keys are held.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    /// Shift.
    pub shift: bool,
    /// Control.
    pub control: bool,
    /// Alt or Option.
    pub alt: bool,
    /// The logo key: Windows, Command or Super.
    pub logo: bool,
}

/// One key going down or up, with the modifiers held.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    /// The key.
    pub key: Key,
    /// The modifiers held at the time.
    pub modifiers: Modifiers,
    /// Whether the key went down rather than up.
    pub pressed: bool,
}

impl Modifiers {
    /// The modifiers a window event says are held.
    pub fn from_window(modifiers: rustle::Modifiers) -> Self {
        Self {
            shift: modifiers.shift,
            control: modifiers.ctrl,
            alt: modifiers.alt,
            logo: modifiers.meta,
        }
    }
}

impl KeyEvent {
    /// The app's key event for `event` from the window, or nothing for a key
    /// the app does not react to, such as a modifier pressed on its own.
    ///
    /// A key that types a character is taken as the character it typed, so
    /// Shift and the keyboard layout are applied, as the state logic expects.
    pub fn from_window(event: &rustle::KeyEvent) -> Option<Self> {
        use rustle::Key as Window;
        let typed = || {
            event
                .text
                .as_deref()
                .and_then(|text| text.chars().next())
                .filter(|character| !character.is_control())
        };
        let key = match &event.key {
            Window::Character(bare) if bare == " " => Key::Space,
            Window::Character(bare) => Key::Character(typed().or_else(|| bare.chars().next())?),
            Window::Enter => Key::Enter,
            Window::Escape => Key::Escape,
            Window::Tab => Key::Tab,
            Window::Backspace => Key::Backspace,
            Window::Delete => Key::Delete,
            Window::Home => Key::Home,
            Window::End => Key::End,
            Window::PageUp => Key::PageUp,
            Window::PageDown => Key::PageDown,
            Window::ArrowUp => Key::ArrowUp,
            Window::ArrowDown => Key::ArrowDown,
            Window::ArrowLeft => Key::ArrowLeft,
            Window::ArrowRight => Key::ArrowRight,
            Window::F(number) if (1..=12).contains(number) => Key::Function(*number),
            _ => return None,
        };
        Some(Self {
            key,
            modifiers: Modifiers::from_window(event.modifiers),
            pressed: event.pressed,
        })
    }
}
