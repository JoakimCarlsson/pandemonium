//! How a file is indented, read off the file rather than set in a preference.
//!
//! A file indented with tabs stays indented with tabs, and one indented two
//! spaces at a time keeps to two: the editor's own habits are not worth a
//! diff on every line somebody else wrote. What it cannot tell from the file
//! — an empty one, or one with no indentation at all — it takes from the
//! reader's own preference.

use ropey::Rope;

/// How wide a step of indentation is when the file does not say.
const FALLBACK_WIDTH: usize = 4;

/// How many lines of a file are read before its habits are called settled.
const SAMPLE: usize = 500;

/// How a file is indented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Indent {
    /// How many columns one step comes to.
    pub width: usize,
    /// Whether a step is written as a tab rather than as spaces.
    pub tabs: bool,
}

impl Default for Indent {
    /// Four spaces, which is the habit a first launch starts from.
    fn default() -> Self {
        Self {
            width: FALLBACK_WIDTH,
            tabs: false,
        }
    }
}

impl Indent {
    /// One step of indentation, as it is written into the text.
    pub fn step(self) -> String {
        if self.tabs {
            "\t".to_owned()
        } else {
            " ".repeat(self.width)
        }
    }

    /// How `text` is indented, judged by the lines that are, and `fallback`
    /// where none of them says.
    ///
    /// Tabs win outright when the file uses them at all often, because a
    /// file that mixes them is a file whose author meant tabs; a tab is then
    /// as wide as `fallback` says. Otherwise the step is the commonest
    /// distance between one line's indentation and the next's, which is the
    /// smallest thing every level is a multiple of.
    pub fn of(text: &Rope, fallback: Self) -> Self {
        let mut tabbed = 0;
        let mut spaced = 0;
        let mut steps = [0usize; 9];
        let mut last = 0usize;

        for line in text.lines().take(SAMPLE) {
            let mut spaces = 0usize;
            let mut tabs = 0usize;
            for ch in line.chars() {
                match ch {
                    ' ' => spaces += 1,
                    '\t' => tabs += 1,
                    _ => break,
                }
            }
            if line.chars().all(char::is_whitespace) {
                continue;
            }
            if tabs > 0 {
                tabbed += 1;
                continue;
            }
            if spaces > 0 {
                spaced += 1;
            }
            if let Some(slot) = spaces
                .checked_sub(last)
                .filter(|step| (1..=8).contains(step))
            {
                steps[slot] += 1;
            }
            last = spaces;
        }

        if tabbed > spaced {
            return Self {
                width: fallback.width,
                tabs: true,
            };
        }
        steps
            .iter()
            .enumerate()
            .skip(1)
            .max_by_key(|(step, count)| (**count, std::cmp::Reverse(*step)))
            .filter(|(_, count)| **count > 0)
            .map_or(fallback, |(width, _)| Self { width, tabs: false })
    }
}
