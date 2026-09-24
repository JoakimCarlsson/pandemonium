//! What a keymap changes over the one beneath it: the bindings it adds and
//! the ones it takes away.
//!
//! A keymap file is one of these over the keymap it extends, and the
//! reader's own bindings are one over whichever keymap they chose. The
//! keymap screen edits the reader's: rebinding an action takes its old
//! chords away and adds the new ones, and putting it back forgets both.

use crate::keymap::action::Action;
use crate::keymap::binding::{Binding, Keymap};
use crate::keymap::chord::Sequence;
use crate::keymap::context::{When, keys};

/// Bindings added over a keymap, and bindings taken out of it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Changes {
    /// The bindings laid over the keymap, lowest precedence first.
    pub bindings: Vec<Binding>,
    /// The sequences taken away from the actions they were bound to.
    pub removed: Vec<(Sequence, Action)>,
}

impl Changes {
    /// Whether nothing is added and nothing taken away.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty() && self.removed.is_empty()
    }

    /// Lays these changes over `keymap`: the removals first, so a binding
    /// taken away and added again elsewhere is added.
    pub fn apply(&self, keymap: &mut Keymap) {
        for (sequence, action) in &self.removed {
            keymap.remove(sequence, *action);
        }
        for binding in &self.bindings {
            keymap.bind(binding.clone());
        }
    }

    /// Whether these changes touch `action` at all.
    pub fn touches(&self, action: Action) -> bool {
        self.bindings
            .iter()
            .any(|binding| binding.action == Some(action))
            || self.removed.iter().any(|(_, removed)| *removed == action)
    }

    /// Binds `action` to `sequence` alone, over `beneath`: whatever
    /// `beneath` presses it as is taken away, and the new binding applies
    /// where the one it replaces did.
    pub fn rebind(&mut self, beneath: &Keymap, action: Action, sequence: Sequence) {
        let when = beneath
            .bindings_of(action)
            .first()
            .map_or_else(|| default_when(action), |binding| binding.when.clone());
        self.clear(beneath, action);
        self.bindings.push(Binding {
            sequence,
            action: Some(action),
            when,
        });
    }

    /// Takes every chord of `action` away, over `beneath`.
    pub fn clear(&mut self, beneath: &Keymap, action: Action) {
        self.bindings
            .retain(|binding| binding.action != Some(action));
        for binding in beneath.bindings_of(action) {
            let removal = (binding.sequence.clone(), action);
            if !self.removed.contains(&removal) {
                self.removed.push(removal);
            }
        }
    }

    /// Forgets everything these changes say about `action`.
    pub fn reset(&mut self, action: Action) {
        self.bindings
            .retain(|binding| binding.action != Some(action));
        self.removed.retain(|(_, removed)| *removed != action);
    }
}

/// Where a binding of `action` applies when nothing beneath says.
fn default_when(action: Action) -> When {
    match action.needs_buffer() {
        true => When::Defined(keys::EDITOR_FOCUSED.to_owned()),
        false => When::Always,
    }
}
