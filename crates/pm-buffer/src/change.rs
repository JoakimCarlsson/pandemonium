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

/// A change made to a text together with the bytes it took away, so that it can be taken back.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Step {
    /// The change.
    pub change: Change,
    /// What the bytes the change replaced held.
    pub removed: String,
}

/// A text before some steps, told from the text after them: what comes before the bytes they touched, what those bytes held, and what comes after.
struct Before<'a> {
    /// The text after the steps.
    after: &'a str,
    /// Where the bytes the steps touched start, alike before and after.
    start: usize,
    /// What those bytes held before.
    old: &'a str,
    /// Where those bytes end in the text after.
    new_end: usize,
}

impl Before<'_> {
    /// How long the text before was.
    fn len(&self) -> usize {
        self.after.len() - (self.new_end - self.start) + self.old.len()
    }

    /// The byte at `at` of the text before.
    fn byte(&self, at: usize) -> u8 {
        let bytes = self.after.as_bytes();
        if at < self.start {
            bytes[at]
        } else if at < self.start + self.old.len() {
            self.old.as_bytes()[at - self.start]
        } else {
            bytes[self.new_end + at - self.start - self.old.len()]
        }
    }

    /// Whether `at` falls between two characters of the text before.
    fn is_char_boundary(&self, at: usize) -> bool {
        at == 0 || at >= self.len() || (self.byte(at) as i8) >= -0x40
    }

    /// The bytes `range` of the text before, which falls between characters.
    fn slice(&self, range: Range<usize>) -> String {
        let pieces = [
            (0, self.start, &self.after[..self.start]),
            (self.start, self.start + self.old.len(), self.old),
            (
                self.start + self.old.len(),
                self.len(),
                &self.after[self.new_end..],
            ),
        ];
        let mut held = String::with_capacity(range.len());
        for (low, high, piece) in pieces {
            let from = range.start.clamp(low, high);
            let to = range.end.clamp(low, high);
            if from < to {
                held.push_str(&piece[from - low..to - low]);
            }
        }
        held
    }
}

impl Step {
    /// The step that takes this one back.
    pub fn inverse(&self) -> Self {
        let start = self.change.range.start;
        Self {
            change: Change {
                range: start..start + self.change.text.len(),
                text: self.removed.clone(),
            },
            removed: self.change.text.clone(),
        }
    }

    /// The one step that does what `steps`, made one after another, did to a text that is `after` once they are made: the stretch [`Change::between`] finds between the text before and after them, worked out from the bytes they touched alone, or nothing when they left the text as it was.
    pub fn compose(steps: &[Step], after: &str) -> Option<Self> {
        let mut cover: Option<(usize, usize, usize)> = None;
        for step in steps {
            let change = &step.change;
            let grown = change.text.len();
            cover = Some(match cover {
                None => (
                    change.range.start,
                    change.range.end,
                    change.range.start + grown,
                ),
                Some((start, old_end, new_end)) => {
                    let high = new_end.max(change.range.end);
                    (
                        start.min(change.range.start),
                        old_end + (high - new_end),
                        high + grown - change.range.len(),
                    )
                }
            });
        }
        let (start, _, new_end) = cover?;
        let mut old = after[start..new_end].to_owned();
        for step in steps.iter().rev() {
            let undo = step.inverse();
            old.replace_range(
                undo.change.range.start - start..undo.change.range.end - start,
                &undo.change.text,
            );
        }
        if old == after[start..new_end] {
            return None;
        }
        let before = Before {
            after,
            start,
            old: &old,
            new_end,
        };
        let mut head = start
            + old
                .bytes()
                .chain(after[new_end..].bytes())
                .zip(after[start..].bytes())
                .take_while(|(left, right)| left == right)
                .count();
        while !before.is_char_boundary(head) || !after.is_char_boundary(head) {
            head -= 1;
        }
        let room = before.len().min(after.len()) - head;
        let shared = after.len() - new_end;
        let mut tail = if room <= shared {
            room
        } else {
            shared
                + old
                    .bytes()
                    .rev()
                    .chain(after[..start].bytes().rev())
                    .zip(after[..new_end].bytes().rev())
                    .take(room - shared)
                    .take_while(|(left, right)| left == right)
                    .count()
        };
        while !before.is_char_boundary(before.len() - tail)
            || !after.is_char_boundary(after.len() - tail)
        {
            tail -= 1;
        }
        Some(Self {
            change: Change {
                range: head..before.len() - tail,
                text: after[head..after.len() - tail].to_owned(),
            },
            removed: before.slice(head..before.len() - tail),
        })
    }
}

/// Tests for steps told as one.
#[cfg(test)]
mod tests {
    use super::{Change, Step};

    /// Stretches of a text, each replaced by a piece of text, one after another.
    type Replacements<'a> = Vec<(std::ops::Range<usize>, &'a str)>;

    /// Makes `steps` to `text` one after another, each with what it took away.
    fn made(text: &str, steps: &[(std::ops::Range<usize>, &str)]) -> (String, Vec<Step>) {
        let mut text = text.to_owned();
        let mut made = Vec::new();
        for (range, inserted) in steps {
            let removed = text[range.clone()].to_owned();
            let change = Change {
                range: range.clone(),
                text: (*inserted).to_owned(),
            };
            change.apply(&mut text);
            made.push(Step { change, removed });
        }
        (text, made)
    }

    /// Steps told as one give what comparing the texts before and after gives.
    #[test]
    fn composed_steps_match_the_stretch_between() {
        let cases: Vec<(&str, Replacements)> = vec![
            ("hello world", vec![(5..5, "!")]),
            ("hello world", vec![(0..5, "hello")]),
            ("hello world", vec![(0..5, "help")]),
            ("aaaa", vec![(2..2, "a")]),
            ("abab", vec![(2..2, "ab")]),
            ("one\ntwo\nthree", vec![(8..13, "3"), (0..3, "1")]),
            ("café 🦀", vec![(3..5, "e"), (4..5, "")]),
            ("xyz", vec![(1..2, "q"), (1..2, "y")]),
            ("é", vec![(0..2, "è")]),
        ];
        for (text, steps) in cases {
            let (after, made) = made(text, &steps);
            let composed = Step::compose(&made, &after);
            let between = Change::between(text, &after);
            assert_eq!(composed.as_ref().map(|step| &step.change), between.as_ref());
            if let Some(step) = composed {
                assert_eq!(&text[step.change.range.clone()], step.removed);
            }
        }
    }
}
