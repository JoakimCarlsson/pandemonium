//! One stretch of a text replaced by other text.

use std::ops::Range;

/// The bytes `range` of a text replaced by `text`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Change {
    /// The bytes of the text before the change that were replaced.
    pub range: Range<usize>,
    /// What replaced them.
    pub text: String,
}

impl Change {
    /// The one stretch that turns `before` into `after`, found as what lies between the start and the end they share, or nothing when they are the same.
    pub fn between(before: &str, after: &str) -> Option<Self> {
        if before == after {
            return None;
        }
        let mut head = before
            .bytes()
            .zip(after.bytes())
            .take_while(|(left, right)| left == right)
            .count();
        while !before.is_char_boundary(head) || !after.is_char_boundary(head) {
            head -= 1;
        }
        let room = before.len().min(after.len()) - head;
        let mut tail = before
            .bytes()
            .rev()
            .zip(after.bytes().rev())
            .take(room)
            .take_while(|(left, right)| left == right)
            .count();
        while !before.is_char_boundary(before.len() - tail)
            || !after.is_char_boundary(after.len() - tail)
        {
            tail -= 1;
        }
        Some(Self {
            range: head..before.len() - tail,
            text: after[head..after.len() - tail].to_owned(),
        })
    }

    /// Where the replacing text ends in the text after the change.
    pub fn new_end(&self) -> usize {
        self.range.start + self.text.len()
    }

    /// Makes the change to `text`.
    pub fn apply(&self, text: &mut String) {
        text.replace_range(self.range.clone(), &self.text);
    }
}
