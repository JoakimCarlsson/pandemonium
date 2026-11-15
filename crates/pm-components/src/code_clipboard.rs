//! What the code editors last put on the system clipboard, remembered so a
//! paste of the same text knows how it was copied: as whole lines, which
//! go above the caret's line, or at an indentation, which the pasted lines
//! are moved from to the place they go.

use std::cell::RefCell;

/// How a text was copied from a code editor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Copied {
    /// The text put on the clipboard.
    pub(crate) text: String,
    /// Whether it is whole lines copied with nothing selected.
    pub(crate) whole_lines: bool,
    /// The spaces and tabs the line it was copied from begins with.
    pub(crate) indentation: String,
}

thread_local! {
    /// The last text a code editor copied, when one did.
    static LAST: RefCell<Option<Copied>> = const { RefCell::new(None) };
}

/// Remembers that `copied` was put on the clipboard.
pub(crate) fn remember_copy(copied: Copied) {
    LAST.with(|last| *last.borrow_mut() = Some(copied));
}

/// How `text`, which the clipboard holds, was copied from a code editor;
/// nothing when it was not the last text an editor copied.
pub(crate) fn copied_as(text: &str) -> Option<Copied> {
    LAST.with(|last| last.borrow().clone().filter(|held| held.text == text))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the text copied last is known; other text was copied elsewhere.
    #[test]
    fn knows_only_its_own_copy() {
        remember_copy(Copied {
            text: "a\n".into(),
            whole_lines: true,
            indentation: String::new(),
        });
        assert!(copied_as("a\n").is_some_and(|copied| copied.whole_lines));
        assert!(copied_as("b").is_none());
    }
}
