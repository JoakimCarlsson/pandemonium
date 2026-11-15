//! What the keys pressed in a terminal send the shell.

use pm_editor::Action;
use rustle::{Key, KeyEvent};

use crate::key_bindings::{Bound, KeyBindings};

/// The modifier number the extended forms of a key carry: one, plus one for shift, two for alt and four for control.
fn modifier_code(event: &KeyEvent) -> u8 {
    1 + u8::from(event.modifiers.shift)
        + 2 * u8::from(event.modifiers.alt)
        + 4 * u8::from(event.modifiers.ctrl)
}

/// The sequence for a key that ends in a letter, such as an arrow, in the plain, the application or the modified form.
fn lettered(event: &KeyEvent, letter: char, application: bool) -> Vec<u8> {
    let code = modifier_code(event);
    if code > 1 {
        format!("\x1b[1;{code}{letter}").into_bytes()
    } else if application {
        format!("\x1bO{letter}").into_bytes()
    } else {
        format!("\x1b[{letter}").into_bytes()
    }
}

/// The sequence for a key that ends in a tilde, such as Delete or Page Up.
fn tilded(event: &KeyEvent, number: u8) -> Vec<u8> {
    let code = modifier_code(event);
    if code > 1 {
        format!("\x1b[{number};{code}~").into_bytes()
    } else {
        format!("\x1b[{number}~").into_bytes()
    }
}

/// The byte a control chord on `character` sends, if it sends one.
fn control_byte(character: char) -> Option<u8> {
    match character {
        'a'..='z' => Some(character as u8 - b'a' + 1),
        'A'..='Z' => Some(character as u8 - b'A' + 1),
        '@' | ' ' => Some(0),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' | '/' => Some(0x1f),
        _ => None,
    }
}

/// The number xterm sends in the escape sequence of the function key F`number`, from F5 up.
fn function_code(number: u8) -> Option<u8> {
    match number {
        5 => Some(15),
        6..=10 => Some(number + 11),
        11 | 12 => Some(number + 12),
        _ => None,
    }
}

/// The bytes a key that types text sends: its control byte under control,
/// otherwise the text it typed, prefixed with escape under alt.
fn typed(event: &KeyEvent) -> Option<Vec<u8>> {
    let text = match (&event.key, &event.text) {
        (Key::Character(unmodified), _) if event.modifiers.ctrl => {
            return Some(vec![control_byte(unmodified.chars().next()?)?]);
        }
        (Key::Character(unmodified), _) if event.modifiers.alt => unmodified.clone(),
        (_, Some(text)) if !text.is_empty() => text.clone(),
        (Key::Character(unmodified), None) => unmodified.clone(),
        _ => return None,
    };
    let text = text.into_bytes();
    Some(if event.modifiers.alt {
        [b"\x1b".as_slice(), &text].concat()
    } else {
        text
    })
}

/// Something a terminal does on its own when asked from its menu or a shortcut, rather than typing into the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalCommand {
    /// Puts the marked text in the composer of the open session.
    AddToChat,
    /// Puts the marked text on the clipboard.
    Copy,
    /// Types the clipboard's text into the shell.
    Paste,
    /// Marks everything kept.
    SelectAll,
    /// Clears the window and what is kept above it.
    Clear,
    /// Ends the shell and drops its tab.
    Kill,
}

/// A shortcut of the terminal itself, caught before the keys go to the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalShortcut {
    /// A command the menu also offers.
    Command(TerminalCommand),
    /// Scrolls a window's height less one line back, when positive, or forward.
    Page(i32),
    /// Part of a binding of several chords, which does nothing yet.
    Swallowed,
}

impl TerminalCommand {
    /// The command the keymap's `action` asks of a terminal, when it is one.
    pub fn of_action(action: Action) -> Option<Self> {
        match action {
            Action::TerminalAddToChat => Some(Self::AddToChat),
            Action::TerminalCopy => Some(Self::Copy),
            Action::TerminalPaste => Some(Self::Paste),
            Action::TerminalSelectAll => Some(Self::SelectAll),
            Action::TerminalClear => Some(Self::Clear),
            Action::KillTerminal => Some(Self::Kill),
            _ => None,
        }
    }

    /// The keymap's action for the command, whose keys its menu row shows.
    pub fn action(self) -> Action {
        match self {
            Self::AddToChat => Action::TerminalAddToChat,
            Self::Copy => Action::TerminalCopy,
            Self::Paste => Action::TerminalPaste,
            Self::SelectAll => Action::TerminalSelectAll,
            Self::Clear => Action::TerminalClear,
            Self::Kill => Action::KillTerminal,
        }
    }

    /// What its menu row reads.
    pub fn label(self) -> &'static str {
        match self {
            Self::AddToChat => "Add to chat",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select all",
            Self::Clear => "Clear",
            Self::Kill => "Kill",
        }
    }

    /// Whether the command acts on the marked text, so does nothing while none is marked.
    pub fn needs_selection(self) -> bool {
        matches!(self, Self::AddToChat | Self::Copy)
    }
}

/// The terminal's own shortcut `event` stands for: the terminal commands the keymap in `bindings` binds to it, and shift with Page Up or Page Down; `None` when the keys are the shell's.
///
/// A chord that starts or breaks off a binding of several is taken as nothing at all, so it reaches neither the shell nor the window.
pub fn terminal_shortcut(event: &KeyEvent, bindings: &KeyBindings) -> Option<TerminalShortcut> {
    if !event.pressed {
        return None;
    }
    match bindings.press_own(event, |action| TerminalCommand::of_action(action).is_some()) {
        Bound::Action(action) => {
            return TerminalCommand::of_action(action).map(TerminalShortcut::Command);
        }
        Bound::Taken => return Some(TerminalShortcut::Swallowed),
        Bound::Nothing => {}
    }
    let modifiers = event.modifiers;
    if modifiers.shift && !modifiers.ctrl && !modifiers.alt {
        return match event.key {
            Key::PageUp => Some(TerminalShortcut::Page(1)),
            Key::PageDown => Some(TerminalShortcut::Page(-1)),
            _ => None,
        };
    }
    None
}

/// The bytes `event` sends a shell, or nothing for a key the shell does not
/// read, a shortcut of the window, or a key coming up.
///
/// `application_cursor` is whether the program asked for the application form of the arrow keys.
pub fn key_bytes(event: &KeyEvent, application_cursor: bool) -> Option<Vec<u8>> {
    if !event.pressed || event.modifiers.meta {
        return None;
    }
    let alt = event.modifiers.alt;
    let control = event.modifiers.ctrl;
    let word_motion = alt && !control && !event.modifiers.shift;
    Some(match event.key {
        Key::Enter => b"\r".to_vec(),
        Key::Escape => b"\x1b".to_vec(),
        Key::Tab if event.modifiers.shift => b"\x1b[Z".to_vec(),
        Key::Tab => b"\t".to_vec(),
        Key::Backspace if control => vec![0x08],
        Key::Backspace if alt => b"\x1b\x7f".to_vec(),
        Key::Backspace => vec![0x7f],
        Key::Insert => tilded(event, 2),
        Key::Delete => tilded(event, 3),
        Key::PageUp => tilded(event, 5),
        Key::PageDown => tilded(event, 6),
        Key::ArrowUp => lettered(event, 'A', application_cursor),
        Key::ArrowDown => lettered(event, 'B', application_cursor),
        Key::ArrowRight if word_motion => b"\x1bf".to_vec(),
        Key::ArrowLeft if word_motion => b"\x1bb".to_vec(),
        Key::ArrowRight => lettered(event, 'C', application_cursor),
        Key::ArrowLeft => lettered(event, 'D', application_cursor),
        Key::Home => lettered(event, 'H', application_cursor),
        Key::End => lettered(event, 'F', application_cursor),
        Key::F(number @ 1..=4) => vec![0x1b, b'O', b'P' + number - 1],
        Key::F(number) => tilded(event, function_code(number)?),
        Key::Shift | Key::Control | Key::Alt | Key::Super => return None,
        _ => typed(event)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustle::Modifiers;

    /// Letters send their text, and under control their control byte.
    #[test]
    fn letters_send_text_or_control_bytes() {
        let plain = KeyEvent::press(Key::Character("a".to_owned()), Modifiers::NONE);
        assert_eq!(key_bytes(&plain, false), Some(b"a".to_vec()));
        let chord = KeyEvent::press(Key::Character("c".to_owned()), Modifiers::CTRL);
        assert_eq!(key_bytes(&chord, false), Some(vec![3]));
    }

    /// Arrows follow the application cursor mode and carry modifiers.
    #[test]
    fn arrows_follow_the_cursor_mode() {
        let up = KeyEvent::press(Key::ArrowUp, Modifiers::NONE);
        assert_eq!(key_bytes(&up, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(key_bytes(&up, true), Some(b"\x1bOA".to_vec()));
        let shifted = KeyEvent::press(Key::ArrowUp, Modifiers::SHIFT);
        assert_eq!(key_bytes(&shifted, false), Some(b"\x1b[1;2A".to_vec()));
    }

    /// The keymap's control and shift with C copies, shift with Page Up pages back, and the window's chords are left to the shell.
    #[test]
    fn shortcuts_are_caught() {
        let copy = KeyEvent::press(
            Key::Character("C".to_owned()),
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        );
        let bindings = KeyBindings::default();
        assert_eq!(
            terminal_shortcut(&copy, &bindings),
            Some(TerminalShortcut::Command(TerminalCommand::Copy))
        );
        let page = KeyEvent::press(Key::PageUp, Modifiers::SHIFT);
        assert_eq!(
            terminal_shortcut(&page, &bindings),
            Some(TerminalShortcut::Page(1))
        );
        let plain = KeyEvent::press(Key::PageUp, Modifiers::NONE);
        assert_eq!(terminal_shortcut(&plain, &bindings), None);
        let window = KeyEvent::press(
            Key::Character("r".to_owned()),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        );
        assert_eq!(terminal_shortcut(&window, &bindings), None);
    }

    /// Enter sends a carriage return and lone modifiers send nothing.
    #[test]
    fn enter_and_modifiers() {
        let enter = KeyEvent::press(Key::Enter, Modifiers::NONE);
        assert_eq!(key_bytes(&enter, false), Some(b"\r".to_vec()));
        let shift = KeyEvent::press(Key::Shift, Modifiers::SHIFT);
        assert_eq!(key_bytes(&shift, false), None);
    }
}
