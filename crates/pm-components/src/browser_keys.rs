//! The keys a page of the browser takes for the browser itself, as a browser's own do, and how every other key is told to the page.

use pm_browser::Modifiers as Held;
use rustle::{Key, KeyEvent, Modifiers};

/// What a key of the browser does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserShortcut {
    /// Shows the bar for finding in the page.
    Find,
    /// Goes to the next match.
    FindNext,
    /// Goes to the previous match.
    FindPrevious,
    /// Zooms the page in one step.
    ZoomIn,
    /// Zooms the page out one step.
    ZoomOut,
    /// Puts the zoom back to 100%.
    ZoomReset,
    /// Loads the page again.
    Reload,
    /// Loads the page again without the cache.
    HardReload,
    /// Opens a new tab.
    NewTab,
    /// Closes the tab shown.
    CloseTab,
    /// Goes back one page, as the back button of the mouse does.
    Back,
    /// Goes forward one page, as the forward button of the mouse does.
    Forward,
}

/// What `event` does in the browser while a page has the keyboard, if it is a key the browser takes: with Control or Command, `F` finds, `G` steps through the matches, `=`, `-` and `0` zoom, `R` reloads and with Shift reloads without the cache, `T` opens a new tab and `W` closes the tab; and `F3`, `F5` and Control `F5` do the same on their own.
pub fn browser_shortcut(event: &KeyEvent) -> Option<BrowserShortcut> {
    if !event.pressed {
        return None;
    }
    let modifiers = event.modifiers;
    let command = (modifiers.ctrl || modifiers.meta) && !modifiers.alt;
    match &event.key {
        Key::Character(text) if command => match text.to_ascii_lowercase().as_str() {
            "f" => Some(BrowserShortcut::Find),
            "g" if modifiers.shift => Some(BrowserShortcut::FindPrevious),
            "g" => Some(BrowserShortcut::FindNext),
            "=" | "+" => Some(BrowserShortcut::ZoomIn),
            "-" | "_" => Some(BrowserShortcut::ZoomOut),
            "0" => Some(BrowserShortcut::ZoomReset),
            "r" if modifiers.shift => Some(BrowserShortcut::HardReload),
            "r" => Some(BrowserShortcut::Reload),
            "t" => Some(BrowserShortcut::NewTab),
            "w" => Some(BrowserShortcut::CloseTab),
            _ => None,
        },
        Key::F(3) if !modifiers.alt && !command => Some(if modifiers.shift {
            BrowserShortcut::FindPrevious
        } else {
            BrowserShortcut::FindNext
        }),
        Key::F(5) if !modifiers.alt => Some(if command || modifiers.shift {
            BrowserShortcut::HardReload
        } else {
            BrowserShortcut::Reload
        }),
        _ => None,
    }
}

/// The engine's modifiers for the keys held in `modifiers`, with `buttons` added for the mouse buttons down.
pub fn held(modifiers: Modifiers, buttons: u32) -> Held {
    let mut bits = buttons;
    if modifiers.shift {
        bits |= Held::SHIFT;
    }
    if modifiers.ctrl {
        bits |= Held::CONTROL;
    }
    if modifiers.alt {
        bits |= Held::ALT;
    }
    if modifiers.meta {
        bits |= Held::COMMAND;
    }
    Held(bits)
}

/// The Windows virtual-key code of a key that types `text`, which is how the engine names keys.
fn character_code(text: &str) -> i32 {
    let Some(character) = text.chars().next() else {
        return 0;
    };
    let upper = character.to_ascii_uppercase();
    match upper {
        'A'..='Z' | '0'..='9' => upper as i32,
        ' ' => 0x20,
        ';' | ':' => 0xBA,
        '=' | '+' => 0xBB,
        ',' | '<' => 0xBC,
        '-' | '_' => 0xBD,
        '.' | '>' => 0xBE,
        '/' | '?' => 0xBF,
        '`' | '~' => 0xC0,
        '[' | '{' => 0xDB,
        '\\' | '|' => 0xDC,
        ']' | '}' => 0xDD,
        '\'' | '"' => 0xDE,
        _ => 0,
    }
}

/// The key `event` as the engine is told it: its virtual-key code, the text it typed when no command key is held, and the modifiers; nothing for a key the engine has no name for.
pub fn page_key(event: &KeyEvent) -> Option<pm_browser::Key> {
    let modifiers = held(event.modifiers, 0);
    let plain = !(event.modifiers.ctrl || event.modifiers.alt || event.modifiers.meta);
    let typed = |fallback: &str| {
        plain.then(|| {
            event
                .text
                .clone()
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| fallback.to_owned())
        })
    };
    let (code, text) = match &event.key {
        Key::Character(character) => (character_code(character), typed(character)),
        Key::Enter => (0x0D, Some("\r".to_owned())),
        Key::Escape => (0x1B, None),
        Key::Tab => (0x09, plain.then(|| "\t".to_owned())),
        Key::Backspace => (0x08, None),
        Key::Delete => (0x2E, None),
        Key::Insert => (0x2D, None),
        Key::Home => (0x24, None),
        Key::End => (0x23, None),
        Key::PageUp => (0x21, None),
        Key::PageDown => (0x22, None),
        Key::ArrowLeft => (0x25, None),
        Key::ArrowUp => (0x26, None),
        Key::ArrowRight => (0x27, None),
        Key::ArrowDown => (0x28, None),
        Key::F(number) if (1..=24).contains(number) => (0x6F + i32::from(*number), None),
        Key::Shift => (0x10, None),
        Key::Control => (0x11, None),
        Key::Alt => (0x12, None),
        Key::ContextMenu => (0x5D, None),
        _ => return None,
    };
    Some(pm_browser::Key {
        pressed: event.pressed,
        code,
        text: if event.pressed { text } else { None },
        modifiers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A press of `key` with `modifiers`, typing `text`.
    fn press(key: Key, modifiers: Modifiers, text: Option<&str>) -> KeyEvent {
        let mut event = KeyEvent::typed(text.unwrap_or_default(), modifiers);
        event.key = key;
        event.text = text.map(str::to_owned);
        event.pressed = true;
        event
    }

    /// Control with a letter is a shortcut of the browser, and the same letter alone is not.
    #[test]
    fn control_letters_are_shortcuts() {
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let ctrl_shift = Modifiers {
            shift: true,
            ..ctrl
        };
        let letter = |text: &str| Key::Character(text.to_owned());
        assert_eq!(
            browser_shortcut(&press(letter("f"), ctrl, None)),
            Some(BrowserShortcut::Find)
        );
        assert_eq!(
            browser_shortcut(&press(letter("r"), ctrl_shift, None)),
            Some(BrowserShortcut::HardReload)
        );
        assert_eq!(
            browser_shortcut(&press(Key::F(5), Modifiers::NONE, None)),
            Some(BrowserShortcut::Reload)
        );
        assert_eq!(
            browser_shortcut(&press(letter("f"), Modifiers::NONE, Some("f"))),
            None
        );
    }

    /// A letter is told by its code with the text it typed; with Control it types nothing.
    #[test]
    fn keys_carry_their_codes() {
        let key = page_key(&press(
            Key::Character("a".to_owned()),
            Modifiers::NONE,
            Some("a"),
        ))
        .expect("a letter has a code");
        assert_eq!((key.code, key.text.as_deref()), (0x41, Some("a")));
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let key = page_key(&press(Key::Character("c".to_owned()), ctrl, None))
            .expect("a letter has a code");
        assert_eq!((key.code, key.text), (0x43, None));
        assert_eq!(key.modifiers.0 & Held::CONTROL, Held::CONTROL);
        let key = page_key(&press(Key::ArrowDown, Modifiers::NONE, None)).expect("an arrow");
        assert_eq!(key.code, 0x28);
    }
}
