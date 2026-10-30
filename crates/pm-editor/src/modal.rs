//! Modal editing in a document: vim's keys carried out on a copy of the text, and what they did to it made to the document as edits.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use pm_text::Edit;
use pm_treesitter::{Syntax, TextObject};
use pm_vim::{
    Buffer, Clipboard, Indent, Keystroke, Outcome, Selection, Shape, SyntaxNode, TypingRules, View,
    Vim,
};

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
    /// Whether an opening bracket or quote that vim replays is written with its closing one.
    pub auto_close: bool,
    /// Whether a line break that vim replays is indented as the language says.
    pub auto_indent: bool,
    /// Whether text vim replays is typed with the rules above, rather than put in as it is.
    pub replay_typing: bool,
    /// Whether functions, classes and comments are found in the syntax tree for vim's objects and motions.
    pub syntax_objects: bool,
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

/// The functions, classes and comments of `text` that its syntax tree holds, as the nodes vim's objects and motions go by, each with what is inside it and, when the outline names it, what it is called.
fn syntax_nodes(syntax: &Syntax, text: &str) -> Vec<SyntaxNode> {
    let buffer = Buffer::new(text);
    let mut names: HashMap<usize, String> = HashMap::new();
    for item in syntax.outline(text) {
        names.entry(item.range.start).or_insert(item.name);
    }
    let places = |range: Range<usize>| {
        buffer.position_of_byte(range.start)..buffer.position_of_byte(range.end)
    };
    syntax
        .text_objects(text)
        .into_iter()
        .map(|span| {
            let (kind, named) = match span.object {
                TextObject::Function => ("function_definition", true),
                TextObject::Class => ("class_definition", true),
                TextObject::Comment => ("comment", false),
            };
            SyntaxNode {
                kind: kind.to_owned(),
                name: names.get(&span.whole.start).cloned().filter(|_| named),
                body: span.inside.map(places),
                range: places(span.whole),
            }
        })
        .collect()
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
    fn modal_buffer(&self, view: ModalView) -> Buffer {
        let mut buffer = Buffer::new(self.field.text());
        buffer.set_indent(Indent {
            width: self.editing.indent.max(1),
            tabs: self.editing.tabs,
        });
        buffer.set_line_comment(self.line_comment_at(self.field.caret()));
        if view.replay_typing {
            let rules = self.editing.indent_rules.clone();
            buffer.set_typing_rules(Some(TypingRules {
                pairs: self.editing.pairs.clone(),
                close: view.auto_close,
                indent: view.auto_indent,
                increases: Some(Arc::new(move |line| rules.increases(line))),
            }));
        }
        if let (true, Some(syntax)) = (view.syntax_objects, &self.syntax) {
            match &self.vim_nodes {
                Some((revision, nodes)) if *revision == syntax.revision() => {
                    buffer.set_syntax_nodes(Arc::clone(nodes));
                }
                _ => {
                    let (syntax, text) = (syntax.clone(), self.field.text().to_owned());
                    buffer.set_syntax_loader(move || syntax_nodes(&syntax, &text));
                }
            }
        }
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
        let mut buffer = self.modal_buffer(view);
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
        if let (Some(syntax), Some(nodes)) = (&self.syntax, buffer.loaded_syntax_nodes()) {
            self.vim_nodes = Some((syntax.revision(), nodes));
        }
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
