//! A binding, and the layered keymap of them a window resolves against.

use std::fmt::{self, Display, Formatter};

use crate::keymap::action::Action;
use crate::keymap::chord::{Chord, Sequence};
use crate::keymap::context::{Context, When};

/// A sequence of chords, what it does and when it does it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Binding {
    /// The chords that trigger the binding.
    pub sequence: Sequence,
    /// What the binding does, or nothing if it unbinds the sequence.
    pub action: Option<Action>,
    /// The condition under which the binding applies.
    pub when: When,
}

impl Binding {
    /// Binds `sequence` to the action named `action`, or to nothing, under
    /// `when`, all written as a keymap spells them.
    ///
    /// The three are parsed together because a binding is only ever written as
    /// the three of them; a failure names the part that could not be read.
    pub fn parse(sequence: &str, action: Option<&str>, when: &str) -> Result<Self, BadBinding> {
        let bad = |row: &str, reason: String| BadBinding {
            row: row.to_owned(),
            reason,
        };
        Ok(Self {
            sequence: sequence
                .parse()
                .map_err(|error| bad(sequence, format!("{error}")))?,
            action: action
                .map(|named| named.parse::<Action>())
                .transpose()
                .map_err(|error| bad(sequence, format!("{error}")))?,
            when: when
                .parse()
                .map_err(|error| bad(when, format!("{error}")))?,
        })
    }

    /// Whether the binding applies in `context`.
    pub fn applies(&self, context: &Context) -> bool {
        self.when.evaluate(context)
    }
}

impl Display for Binding {
    /// Writes the binding the way a keymap spells it.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self.action {
            Some(action) => write!(formatter, "{} → {action}", self.sequence),
            None => write!(formatter, "{} → unbound", self.sequence),
        }?;
        match self.when {
            When::Always => Ok(()),
            ref when => write!(formatter, " when {when}"),
        }
    }
}

/// A row of a keymap table that could not be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadBinding {
    /// The text that could not be read.
    pub row: String,
    /// Why it could not be read.
    pub reason: String,
}

impl Display for BadBinding {
    /// Names the row and says why it could not be read.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "bad binding `{}`: {}", self.row, self.reason)
    }
}

impl std::error::Error for BadBinding {}

/// Every binding the window resolves against, in order of precedence.
///
/// Later bindings win: a keymap is built by laying the keymap it extends
/// down, its own bindings over that, and the reader's own over those.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    /// The bindings, lowest precedence first.
    bindings: Vec<Binding>,
}

impl Keymap {
    /// An empty keymap.
    pub fn new() -> Self {
        Self::default()
    }

    /// The keymap of `bindings`, lowest precedence first.
    pub fn of(bindings: Vec<Binding>) -> Self {
        Self { bindings }
    }

    /// Adds `binding` above everything already in the keymap.
    pub fn bind(&mut self, binding: Binding) -> &mut Self {
        self.bindings.push(binding);
        self
    }

    /// Takes `sequence` out of service above everything already in the keymap.
    pub fn unbind(&mut self, sequence: Sequence, when: When) -> &mut Self {
        self.bind(Binding {
            sequence,
            action: None,
            when,
        })
    }

    /// Lays `other` over this keymap, so its bindings win.
    pub fn layer(&mut self, other: Self) -> &mut Self {
        self.bindings.extend(other.bindings);
        self
    }

    /// Takes every binding of `sequence` to `action` out, whatever its
    /// `when`, leaving what the sequence does elsewhere alone.
    pub fn remove(&mut self, sequence: &Sequence, action: Action) -> &mut Self {
        self.bindings
            .retain(|binding| !(binding.sequence == *sequence && binding.action == Some(action)));
        self
    }

    /// The keymap with `key` known to carry `value`: every clause is
    /// settled against it, and a binding that can then never apply is left
    /// out.
    pub fn settle(self, key: &str, value: &str) -> Self {
        let bindings = self
            .bindings
            .into_iter()
            .map(|binding| Binding {
                when: binding.when.settle(key, value),
                ..binding
            })
            .filter(|binding| binding.when != When::Never)
            .collect();
        Self { bindings }
    }

    /// Every binding, lowest precedence first.
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// The bindings `pressed` is a prefix of, that apply in `context`.
    pub fn candidates<'a>(
        &'a self,
        pressed: &'a [Chord],
        context: &'a Context,
    ) -> impl Iterator<Item = &'a Binding> {
        self.bindings.iter().filter(move |binding| {
            binding.sequence.starts_with(pressed) && binding.applies(context)
        })
    }

    /// Every binding of `action` still in force somewhere: one a later
    /// binding of the same sequence under the same clause has not replaced.
    pub fn bindings_of(&self, action: Action) -> Vec<&Binding> {
        self.bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| binding.action == Some(action))
            .filter(|(at, binding)| {
                !self.bindings[at + 1..]
                    .iter()
                    .any(|later| later.sequence == binding.sequence && later.when == binding.when)
            })
            .map(|(_, binding)| binding)
            .collect()
    }

    /// The chords `action` is pressed as in `context`, as a palette shows them.
    pub fn sequence_for(&self, action: Action, context: &Context) -> Option<&Sequence> {
        self.bindings
            .iter()
            .rev()
            .find(|binding| binding.action == Some(action) && binding.applies(context))
            .map(|binding| &binding.sequence)
    }
}
