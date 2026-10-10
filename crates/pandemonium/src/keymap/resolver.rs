//! Turning the chords pressed so far into an action.

use crate::keymap::action::Action;
use crate::keymap::binding::Keymap;
use crate::keymap::chord::Chord;
use crate::keymap::context::Context;

/// What a keypress came to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resolution {
    /// Nothing is bound to the chords pressed; they are forgotten.
    None,
    /// The chords so far are the start of a binding; the next one finishes it.
    Pending,
    /// The chords pressed are bound to this action.
    Act(Action),
}

/// A keymap and the chords pressed into it so far.
///
/// A binding of several chords is only recognised once its last chord is
/// pressed; until then the resolver holds what came before, so `ctrl+k ctrl+c`
/// is one binding and not two keypresses that happen to follow each other.
#[derive(Clone, Debug, Default)]
pub struct Resolver {
    /// The keymap in force.
    keymap: Keymap,
    /// The chords pressed since the last binding matched or failed.
    pending: Vec<Chord>,
}

impl Resolver {
    /// A resolver over `keymap`, with nothing pressed yet.
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            pending: Vec::new(),
        }
    }

    /// The keymap in force.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Puts `keymap` in force and forgets what was pressed.
    pub fn set_keymap(&mut self, keymap: Keymap) {
        self.keymap = keymap;
        self.pending.clear();
    }

    /// The chords pressed so far, which a window can show while it waits.
    pub fn pending(&self) -> &[Chord] {
        &self.pending
    }

    /// Forgets the chords pressed so far.
    pub fn reset(&mut self) {
        self.pending.clear();
    }

    /// Folds `chord` into what is pressed and says what it came to.
    pub fn press(&mut self, chord: Chord, context: &Context) -> Resolution {
        let mut pressed = std::mem::take(&mut self.pending);
        pressed.push(chord);

        let resolution = self.resolve(&pressed, context);
        if resolution == Resolution::Pending {
            self.pending = pressed;
        }
        resolution
    }

    /// Resolves a chord without consuming the pending sequence.
    pub fn preview(&self, chord: Chord, context: &Context) -> Resolution {
        let mut pressed = self.pending.clone();
        pressed.push(chord);
        self.resolve(&pressed, context)
    }

    /// Resolves a complete pressed sequence against the active keymap.
    fn resolve(&self, pressed: &[Chord], context: &Context) -> Resolution {
        let Some(binding) = self.keymap.candidates(pressed, context).last() else {
            return Resolution::None;
        };
        if binding.sequence.len() > pressed.len() {
            return Resolution::Pending;
        }
        match binding.action {
            Some(action) => Resolution::Act(action),
            None => Resolution::None,
        }
    }
}
