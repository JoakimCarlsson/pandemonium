//! The keymap handed down to the controls that hold the keyboard, the code
//! editor and the terminal among them, so the keys the reader bound work
//! wherever the keyboard is: each control looks a chord up among the actions
//! it does itself and the actions that work everywhere, follows bindings of
//! two chords across presses, and hands the actions that work everywhere
//! back to the window.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use pm_editor::{Action, Chord, Keymap, Preset, Pressed};
use rustle::{CompositionLocal, KeyEvent};

use crate::key_recorder::KeyRecorder;

/// What a chord pressed in a control came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    /// The chords pressed are bound to this action of the control's own.
    Action(Action),
    /// The chords pressed started a binding of more, or finished one the window does; the control takes the key and does nothing more with it.
    Taken,
    /// The keymap binds nothing to the chord; the control handles it as it would without a keymap.
    Nothing,
}

/// The keymap the window hands down, with the chords pressed so far of a binding of several and where the actions that work everywhere go.
#[derive(Clone)]
pub struct KeyBindings {
    /// The bindings.
    keymap: Rc<Keymap>,
    /// The chords pressed so far of a binding of several, shared by every control.
    pending: Rc<RefCell<Vec<Chord>>>,
    /// What does an action that works everywhere.
    window: Rc<dyn Fn(Action)>,
}

impl PartialEq for KeyBindings {
    /// Whether both hold the same bindings; where they send window actions is not compared.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.keymap, &other.keymap) || self.keymap == other.keymap
    }
}

impl Default for KeyBindings {
    /// The bindings of the default preset, with the window's actions going nowhere, as in snapshots.
    fn default() -> Self {
        Self::new(Keymap::new(Preset::default(), &BTreeMap::new()), |_| {})
    }
}

impl KeyBindings {
    /// The bindings of `keymap`, handing the actions that work everywhere to `window`.
    pub fn new(keymap: Keymap, window: impl Fn(Action) + 'static) -> Self {
        Self {
            keymap: Rc::new(keymap),
            pending: Rc::new(RefCell::new(Vec::new())),
            window: Rc::new(window),
        }
    }

    /// The keymap.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Looks the press `event` up among the actions `own` accepts and those that work everywhere, after the chords pressed before it.
    ///
    /// A binding of the window is done at once through the window's handler and comes back as [`Bound::Taken`], so the window's sequences work while a control has the keyboard. A chord that breaks off a sequence is taken too, so it does not type.
    pub fn press(&self, event: &KeyEvent, own: impl Fn(Action) -> bool) -> Bound {
        self.press_with(event, own, true)
    }

    /// Looks the press `event` up among the actions `own` accepts alone, after the chords pressed before it, leaving the window's bindings to the control; for a shell, which reads those chords itself.
    pub fn press_own(&self, event: &KeyEvent, own: impl Fn(Action) -> bool) -> Bound {
        self.press_with(event, own, false)
    }

    /// Looks the press `event` up among the actions `own` accepts, and those that work everywhere when `window`, after the chords pressed before it.
    fn press_with(&self, event: &KeyEvent, own: impl Fn(Action) -> bool, window: bool) -> Bound {
        if !event.pressed {
            return Bound::Nothing;
        }
        let Some(chord) = chord_of(event) else {
            return Bound::Nothing;
        };
        let before = std::mem::take(&mut *self.pending.borrow_mut());
        let among = |action: Action| own(action) || (window && action.everywhere());
        match self.keymap.press_among(&before, &chord, among) {
            Pressed::Waiting => {
                let mut pending = self.pending.borrow_mut();
                *pending = before;
                pending.push(chord);
                Bound::Taken
            }
            Pressed::Action(action) if own(action) => Bound::Action(action),
            Pressed::Action(action) => {
                (self.window)(action);
                Bound::Taken
            }
            Pressed::Nothing if before.is_empty() => Bound::Nothing,
            Pressed::Nothing => Bound::Taken,
        }
    }
}

/// The keymap provided to the composables below, if one was.
static LOCAL_KEY_BINDINGS: CompositionLocal<KeyBindings> =
    CompositionLocal::new(KeyBindings::default);

/// Runs `content` with `bindings` as the keymap of the controls in it.
pub fn provide_key_bindings<R>(bindings: KeyBindings, content: impl FnOnce() -> R) -> R {
    LOCAL_KEY_BINDINGS.provide(bindings, content)
}

/// The keymap provided to the composable calling it, or the default one.
pub fn key_bindings() -> KeyBindings {
    LOCAL_KEY_BINDINGS.current()
}

/// The chord the press `event` makes, as the keymap writes chords.
pub fn chord_of(event: &KeyEvent) -> Option<Chord> {
    KeyRecorder::chord(event).and_then(|written| Chord::parse(&written))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use rustle::{Key, Modifiers};

    use super::*;

    /// A press of the letter `letter` with control held.
    fn control(letter: &str) -> KeyEvent {
        KeyEvent::press(
            Key::Character(letter.to_owned()),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        )
    }

    /// An editor action bound to a chord comes back as the editor's, a window action is handed to the window, and a chord bound to nothing is left alone.
    #[test]
    fn chords_reach_their_owner() {
        let handed = Rc::new(Cell::new(None));
        let seen = handed.clone();
        let bindings = KeyBindings::new(
            Keymap::new(Preset::default(), &BTreeMap::new()),
            move |action| seen.set(Some(action)),
        );
        let editor = |action: Action| !action.everywhere() && !action.in_terminal();
        assert_eq!(
            bindings.press(&control("/"), editor),
            Bound::Action(Action::ToggleComment)
        );
        assert_eq!(bindings.press(&control("`"), editor), Bound::Taken);
        assert_eq!(handed.get(), Some(Action::ToggleTerminal));
        assert_eq!(bindings.press(&control("ü"), editor), Bound::Nothing);
    }

    /// A binding the reader made replaces the preset's in the controls too.
    #[test]
    fn own_bindings_reach_the_editor() {
        let mut own = BTreeMap::new();
        own.insert(
            Action::ToggleComment.key().to_owned(),
            vec!["ctrl+alt+c".to_owned()],
        );
        let bindings = KeyBindings::new(Keymap::new(Preset::default(), &own), |_| {});
        let editor = |action: Action| !action.everywhere() && !action.in_terminal();
        assert_eq!(bindings.press(&control("/"), editor), Bound::Nothing);
        let chord = KeyEvent::press(
            Key::Character("c".to_owned()),
            Modifiers {
                ctrl: true,
                alt: true,
                ..Modifiers::NONE
            },
        );
        assert_eq!(
            bindings.press(&chord, editor),
            Bound::Action(Action::ToggleComment)
        );
    }
}
