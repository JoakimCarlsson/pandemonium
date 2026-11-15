//! A button that, once pressed, records the next chord typed as a binding.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, KeyCode, KeyEvent, PhysicalKey};

use crate::button::{Button, ButtonVariant};
use crate::key_cap::{KeyCap, KeyCapVariant};
use crate::size::Size;

/// How a key recorder is drawn while it waits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyRecorderVariant {
    /// A ghost button reading "Record".
    #[default]
    Ghost,
    /// A raised button.
    Secondary,
}

/// A key recorder with a variant and a size.
#[derive(Clone)]
pub struct KeyRecorder {
    /// Whether it is recording.
    recording: bool,
    /// How it is drawn while it waits.
    variant: KeyRecorderVariant,
    /// Its size.
    size: Size,
    /// What pressing it while it waits does.
    on_start: Option<Rc<dyn Fn()>>,
    /// What a chord typed while it records does, given the chord as the settings write it, such as `ctrl+shift+k`.
    on_record: Option<Rc<dyn Fn(String)>>,
    /// What Escape while it records does.
    on_cancel: Option<Rc<dyn Fn()>>,
}

/// The sign the key of `event` types without Shift on a US layout, judged by where the key sits, for keys whose own text Control turned into a control character.
fn sign_of(event: &KeyEvent) -> Option<String> {
    let PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };
    let sign = match code {
        KeyCode::Backquote => "`",
        KeyCode::Minus => "-",
        KeyCode::Equal => "=",
        KeyCode::BracketLeft => "[",
        KeyCode::BracketRight => "]",
        KeyCode::Backslash => "\\",
        KeyCode::Semicolon => ";",
        KeyCode::Quote => "'",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",
        _ => return None,
    };
    Some(sign.to_owned())
}

/// What the key `event` pressed types on a US layout with no modifier, by where it sits on the keyboard: its letter, digit or sign, or nothing for any other key.
fn us_name(event: &KeyEvent) -> Option<String> {
    let PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };
    let name = code.name();
    if let Some(letter) = name.strip_prefix("Key").filter(|rest| rest.len() == 1) {
        return Some(letter.to_lowercase());
    }
    if let Some(digit) = name.strip_prefix("Digit") {
        return Some(digit.to_owned());
    }
    sign_of(event)
}

/// Whether `event`, pressed with Alt, typed a sign of its own rather than the key's own sign, as AltGr and the Option key do, so it is typing and not a chord.
fn types_beside_alt(event: &KeyEvent) -> bool {
    let Some(typed) = event
        .text
        .as_deref()
        .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
    else {
        return false;
    };
    us_name(event).is_some_and(|own| typed.to_lowercase() != own)
}

impl KeyRecorder {
    /// A recorder that is `recording` or waits.
    pub fn new(recording: bool) -> Self {
        Self {
            recording,
            variant: KeyRecorderVariant::Ghost,
            size: Size::Xs,
            on_start: None,
            on_record: None,
            on_cancel: None,
        }
    }

    /// The recorder in `variant`.
    pub fn variant(mut self, variant: KeyRecorderVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The recorder at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The recorder calling `on_start` when pressed while it waits.
    pub fn on_start(mut self, on_start: impl Fn() + 'static) -> Self {
        self.on_start = Some(Rc::new(on_start));
        self
    }

    /// The recorder calling `on_record` with the chord typed while it records.
    pub fn on_record(mut self, on_record: impl Fn(String) + 'static) -> Self {
        self.on_record = Some(Rc::new(on_record));
        self
    }

    /// The recorder calling `on_cancel` when Escape is pressed while it records.
    pub fn on_cancel(mut self, on_cancel: impl Fn() + 'static) -> Self {
        self.on_cancel = Some(Rc::new(on_cancel));
        self
    }

    /// The chord `event` types, as the settings write it, or nothing for a modifier alone, a key no chord names, or a sign typed with AltGr or Option. With Control, Alt or the command key held, a letter, digit or sign is named by where its key sits, as it reads on a US layout, so chords stay on the same keys whatever the layout.
    pub fn chord(event: &KeyEvent) -> Option<String> {
        let held = event.modifiers;
        if held.alt && types_beside_alt(event) {
            return None;
        }
        let commanding = held.ctrl || held.meta || held.alt;
        let key = match &event.key {
            Key::Character(text) if text == " " => "space".to_owned(),
            Key::Character(_) | Key::Other if commanding && us_name(event).is_some() => {
                us_name(event)?
            }
            Key::Character(text) if text.chars().any(char::is_control) => sign_of(event)?,
            Key::Character(text) => text.to_lowercase(),
            Key::Other => sign_of(event)?,
            Key::Enter => "enter".to_owned(),
            Key::Tab => "tab".to_owned(),
            Key::Backspace => "backspace".to_owned(),
            Key::Delete => "delete".to_owned(),
            Key::ArrowLeft => "left".to_owned(),
            Key::ArrowRight => "right".to_owned(),
            Key::ArrowUp => "up".to_owned(),
            Key::ArrowDown => "down".to_owned(),
            Key::Home => "home".to_owned(),
            Key::End => "end".to_owned(),
            Key::PageUp => "pageup".to_owned(),
            Key::PageDown => "pagedown".to_owned(),
            Key::Insert => "insert".to_owned(),
            Key::F(number) => format!("f{number}"),
            _ => return None,
        };
        let modifiers = event.modifiers;
        let mut parts = Vec::new();
        if modifiers.ctrl || modifiers.meta {
            parts.push("ctrl".to_owned());
        }
        if modifiers.alt {
            parts.push("alt".to_owned());
        }
        if modifiers.shift {
            parts.push("shift".to_owned());
        }
        parts.push(key);
        Some(parts.join("+"))
    }

    /// Emits the recorder.
    #[track_caller]
    pub fn show(self) {
        let focus = remember_focus();
        let asked = remember(|| false);
        if !self.recording {
            if asked.get() {
                asked.set(false);
            }
            let mut button = Button::new("Record")
                .size(self.size)
                .variant(match self.variant {
                    KeyRecorderVariant::Ghost => ButtonVariant::Ghost,
                    KeyRecorderVariant::Secondary => ButtonVariant::Secondary,
                });
            if let Some(on_start) = self.on_start {
                button = button.on_click(move || on_start());
            }
            button.show();
            return;
        }
        if !asked.get() {
            asked.set(true);
            focus.request_focus();
        }
        let on_record = self.on_record;
        let on_cancel = self.on_cancel;
        let size = self.size;
        stack_with(
            Modifier::new().focusable(&focus, move |event| {
                if !event.pressed {
                    return false;
                }
                if event.key == Key::Escape {
                    if let Some(on_cancel) = &on_cancel {
                        on_cancel();
                    }
                    return true;
                }
                match (Self::chord(event), &on_record) {
                    (Some(chord), Some(on_record)) => {
                        on_record(chord);
                        true
                    }
                    _ => false,
                }
            }),
            move || {
                KeyCap::new("Press keys…")
                    .variant(KeyCapVariant::Recording)
                    .size(size)
                    .show();
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use rustle::Modifiers;

    use super::*;

    /// A press of the key at `code` typing `text`, with `modifiers` held.
    fn press(code: KeyCode, text: &str, modifiers: Modifiers) -> KeyEvent {
        KeyEvent {
            physical_key: PhysicalKey::Code(code),
            ..KeyEvent::press(Key::Character(text.to_owned()), modifiers)
        }
    }

    /// Control with the backquote key names the backquote even when it typed a control character, and letters are named by what they type.
    #[test]
    fn modified_signs_are_named_by_their_key() {
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let both = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            KeyRecorder::chord(&press(KeyCode::Backquote, "\0", ctrl)).as_deref(),
            Some("ctrl+`")
        );
        assert_eq!(
            KeyRecorder::chord(&press(KeyCode::KeyP, "P", both)).as_deref(),
            Some("ctrl+shift+p")
        );
    }

    /// With Control held, a key is named by where it sits, so Control with the key left of 1 is the backquote on a Nordic layout too, and Control with Shift and the bracket key names the bracket.
    #[test]
    fn chords_follow_the_physical_key() {
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let both = Modifiers {
            shift: true,
            ..ctrl
        };
        assert_eq!(
            KeyRecorder::chord(&press(KeyCode::Backquote, "§", ctrl)).as_deref(),
            Some("ctrl+`")
        );
        assert_eq!(
            KeyRecorder::chord(&press(KeyCode::BracketLeft, "{", both)).as_deref(),
            Some("ctrl+shift+[")
        );
    }

    /// A sign typed with AltGr is typing, not a chord.
    #[test]
    fn alt_graph_types() {
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        let typed = KeyEvent {
            text: Some("@".to_owned()),
            ..press(KeyCode::Digit2, "@", alt)
        };
        assert_eq!(KeyRecorder::chord(&typed), None);
    }
}
