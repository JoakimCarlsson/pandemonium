//! Modal editing in a document: vim's keys carried out on a copy of the text, and what they did to it made to the document as edits.

use std::ops::Range;

use pm_text::Edit;
use pm_vim::{Buffer, Clipboard, Indent, Keystroke, Outcome, Selection, Shape, View, Vim};

use crate::document::Document;

/// What the pane the document is shown in says about itself, for the motions that go by it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ModalView {
    /// The first line the pane shows.
    pub top: usize,
    /// How many lines the pane has room for.
    pub rows: usize,
    /// How many lines the pane keeps between the caret and its edges.
    pub margin: usize,
    /// The width `gq` wraps text at.
    pub wrap: usize,
}

/// What a key did in modal editing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModalPress {
    /// Whether the key was taken, and what the window has to do because of it.
    pub outcome: Outcome,
    /// The edits that bring the document to where the key left the text and the carets.
    pub edits: Vec<Edit>,
    /// Where undo stood when typing began, once the key has ended it: everything done since is one step, as vim takes back a whole insert at once. Made after the edits are.
    pub squash: Option<usize>,
}

/// The byte range of `old` that `new` replaces, and what it is replaced with, when the two differ.
fn replacement(old: &str, new: &str) -> Option<(Range<usize>, String)> {
    if old == new {
        return None;
    }
    let mut start = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
        start -= 1;
    }
    let mut common = old[start..]
        .bytes()
        .rev()
        .zip(new[start..].bytes().rev())
        .take_while(|(left, right)| left == right)
        .count();
    while !old.is_char_boundary(old.len() - common) || !new.is_char_boundary(new.len() - common) {
        common -= 1;
    }
    Some((
        start..old.len() - common,
        new[start..new.len() - common].to_owned(),
    ))
}

impl Document {
    /// The copy of the text modal editing works on, with the carets of the document on it, the primary last.
    fn modal_buffer(&self) -> Buffer {
        let mut buffer = Buffer::new(self.field.text());
        buffer.set_indent(Indent {
            width: self.editing.indent.max(1),
            tabs: self.editing.tabs,
        });
        buffer.set_line_comment(self.line_comment_at(self.field.caret()));
        let primary = (self.field.anchor(), self.field.caret());
        let mut selections: Vec<Selection> = self
            .field
            .selections()
            .into_iter()
            .filter(|held| *held != primary)
            .map(|(anchor, caret)| Selection {
                anchor: buffer.position_of_byte(anchor),
                head: buffer.position_of_byte(caret),
            })
            .collect();
        selections.push(Selection {
            anchor: buffer.position_of_byte(primary.0),
            head: buffer.position_of_byte(primary.1),
        });
        buffer.set_selections(selections);
        buffer
    }

    /// Carries `key` out in modal editing and says what it did: whether it was taken, what the window has to do, and the edits that make the document what the key left the text and the carets as. Undo and redo are left to the document, which keeps the history.
    pub fn vim_press(
        &mut self,
        vim: &mut Vim,
        key: Keystroke,
        view: ModalView,
        clipboard: &mut dyn Clipboard,
    ) -> ModalPress {
        let mut buffer = self.modal_buffer();
        let folds = self.hidden_lines();
        let typing_before = self.vim.mode().is_typing();
        let depth = self.field.undo_depth();
        let mut state = std::mem::take(&mut self.vim);
        let outcome = vim.press(
            &mut state,
            &mut buffer,
            View {
                top: view.top,
                rows: view.rows,
                margin: view.margin,
                wrap: view.wrap,
                folds: &folds,
            },
            clipboard,
            key,
        );
        self.vim = state;
        let typing_after = self.vim.mode().is_typing();
        if typing_after && !typing_before {
            self.vim_typing_from = Some(depth);
        }
        let squash = if !typing_after && typing_before {
            self.vim_typing_from.take()
        } else {
            None
        };
        if !outcome.handled {
            return ModalPress {
                outcome,
                edits: Vec::new(),
                squash,
            };
        }
        let (undos, redos) = buffer.take_history();
        if undos + redos > 0 {
            let edits = std::iter::repeat_n(Edit::Undo, undos)
                .chain(std::iter::repeat_n(Edit::Redo, redos))
                .collect();
            return ModalPress {
                outcome,
                edits,
                squash,
            };
        }
        let primary = buffer.selection();
        let carets = |held: Selection| (buffer.byte_of(held.anchor), buffer.byte_of(held.head));
        let main = carets(primary);
        let others: Vec<(usize, usize)> = {
            let mut skipped = false;
            buffer
                .selections()
                .into_iter()
                .filter(|held| {
                    let is_primary = !skipped && *held == primary;
                    skipped |= is_primary;
                    !is_primary
                })
                .map(carets)
                .collect()
        };
        let change = replacement(self.field.text(), &buffer.contents());
        let mut now = self.field.selections();
        let mut then: Vec<(usize, usize)> = others.iter().copied().chain([main]).collect();
        now.sort_unstable();
        then.sort_unstable();
        let mut edits = Vec::new();
        let rewrote = change.is_some();
        if let Some((range, text)) = change {
            edits.push(Edit::Replace(range, text, main));
        }
        if rewrote || now != then || (self.field.anchor(), self.field.caret()) != main {
            if others.is_empty() {
                edits.push(Edit::OnlySelection);
            }
            edits.push(Edit::Select(main.0, main.1));
            edits.extend(
                others
                    .into_iter()
                    .map(|(anchor, caret)| Edit::AddSelection(anchor, caret)),
            );
        }
        ModalPress {
            outcome,
            edits,
            squash,
        }
    }

    /// How the caret is drawn while modal editing is on.
    pub fn vim_shape(&self) -> Shape {
        self.vim.shape()
    }

    /// The mode modal editing is in.
    pub fn vim_mode(&self) -> pm_vim::Mode {
        self.vim.mode()
    }

    /// What has been typed towards a command or the line being typed, as a status bar shows it.
    pub fn vim_pending(&self) -> Option<String> {
        self.vim.pending()
    }

    /// Puts modal editing back in normal mode with nothing pending, as when it is turned off.
    pub fn vim_reset(&mut self) {
        self.vim = pm_vim::State::default();
        self.vim_typing_from = None;
    }
}
