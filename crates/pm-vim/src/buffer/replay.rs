//! Typing replayed as live typing is: the rules the window hands the buffer for closing pairs, stepping over closers and indenting new lines, and the buffer's typing that follows them.
//!
//! Text vim types again, for a count, for `.` and for a macro, has no window behind it to pair a bracket or indent a line, so the window gives the buffer what the language and the settings say and the buffer does the same with them.

use std::sync::Arc;

use super::{Buffer, Position};

/// A test of a line of text, as the language's indent patterns make it.
pub type LineRule = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// What the window knows of typing in the file's language, for the buffer to type by when it replays keys.
#[derive(Clone, Default)]
pub struct TypingRules {
    /// The opening and closing character of each pair the language writes together.
    pub pairs: Vec<(char, char)>,
    /// Whether an opening character is written with its closing one.
    pub close: bool,
    /// Whether a new line is indented as the line before it is, one level deeper after a block opens.
    pub indent: bool,
    /// Whether a line, as the language's own patterns say, is one the line after it is indented below.
    pub increases: Option<LineRule>,
}

/// Whether a pair may be opened before `next`, the character after the caret: at the end of a line, before space, or before something that closes or separates.
fn opens_before(next: Option<char>) -> bool {
    next.is_none_or(|next| {
        next.is_whitespace() || matches!(next, ')' | ']' | '}' | ',' | ';' | ':' | '.' | '>')
    })
}

impl Buffer {
    /// Sets the rules replayed typing follows; with none, it puts text in as it is.
    pub fn set_typing_rules(&mut self, rules: Option<TypingRules>) {
        self.typing = rules;
    }

    /// The character before the head of the primary selection, on its line.
    fn char_before_head(&self) -> Option<char> {
        let head = self.selection.head;
        head.column
            .checked_sub(1)
            .and_then(|column| self.char_at(Position::new(head.line, column)))
    }

    /// Types `ch` as the language pairs it, when it is typed at an empty caret: over a closer that is next, or with its closer when it opens a pair; true when it did.
    pub(super) fn type_paired(&mut self, ch: char) -> bool {
        let Some(rules) = self.typing.clone() else {
            return false;
        };
        let head = self.selection.head;
        let next = self.char_at(head);
        let over = Position::new(head.line, head.column + 1);
        if rules.pairs.iter().any(|(_, close)| *close == ch) && next == Some(ch) {
            self.place(over, false);
            return true;
        }
        let Some((open, close)) = rules.pairs.iter().copied().find(|(open, _)| *open == ch) else {
            return false;
        };
        if !rules.close || !opens_before(next) {
            return false;
        }
        let before = self.char_before_head();
        if open == close && before.is_some_and(|before| before.is_alphanumeric() || before == open)
        {
            return false;
        }
        self.replace(head..head, &format!("{open}{close}"));
        self.place(over, false);
        true
    }

    /// Takes a closing bracket just typed on a line that holds nothing else back one level of indentation.
    pub(super) fn outdent_closer(&mut self) {
        let head = self.selection.head;
        let Some(column) = head.column.checked_sub(1) else {
            return;
        };
        let leading: String = self.line_chars(head.line).take(column).collect();
        let unit = self.indent.step();
        let closes = self.char_at(Position::new(head.line, column));
        if !matches!(closes, Some('}' | ')' | ']'))
            || leading.is_empty()
            || !leading.chars().all(|ch| ch == ' ' || ch == '\t')
            || !leading.ends_with(&unit)
        {
            return;
        }
        let width = unit.chars().count();
        self.replace(
            Position::new(head.line, column - width)..Position::new(head.line, column),
            "",
        );
        self.place(Position::new(head.line, column + 1 - width), false);
    }

    /// Puts a line break in as the language indents it: one level deeper after a block opens, with a closing bracket that follows put on a line of its own. With no rules it indents as the line it leaves is.
    pub(super) fn break_line(&mut self) {
        let head = self.selection.start();
        let base: String = self
            .line_chars(head.line)
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .take(head.column)
            .collect();
        let Some(rules) = self.typing.clone() else {
            return self.insert(&format!("\n{base}"));
        };
        if !rules.indent {
            return self.insert("\n");
        }
        let before: String = self.line_chars(head.line).take(head.column).collect();
        let before = before.trim_end();
        let deeper = before.ends_with(['{', '(', '[', ':'])
            || rules.increases.as_ref().is_some_and(|rule| rule(before));
        let closing = deeper
            && self
                .selection
                .is_empty()
                .then(|| {
                    self.line_chars(head.line)
                        .skip(head.column)
                        .find(|ch| *ch != ' ' && *ch != '\t')
                })
                .flatten()
                .is_some_and(|ch| matches!(ch, '}' | ')' | ']'));
        let inner = match deeper {
            true => format!("{base}{}", self.indent.step()),
            false => base.clone(),
        };
        let place = Position::new(head.line + 1, inner.chars().count());
        match closing {
            true => {
                self.insert(&format!("\n{inner}\n{base}"));
                self.place(place, false);
            }
            false => self.insert(&format!("\n{inner}")),
        }
    }

    /// Takes out what backspace does more than one character of: an empty pair whole, or, inside the indentation of a line of spaces, a level of it; false when it is not one of those.
    pub(super) fn backspace_paired(&mut self) -> bool {
        let Some(rules) = self.typing.clone() else {
            return false;
        };
        if !self.selection.is_empty() {
            return false;
        }
        let head = self.selection.head;
        let (before, next) = (self.char_before_head(), self.char_at(head));
        if let (Some(before), Some(next)) = (before, next)
            && rules.pairs.contains(&(before, next))
        {
            let start = Position::new(head.line, head.column - 1);
            self.replace(start..Position::new(head.line, head.column + 1), "");
            return true;
        }
        let leading: String = self.line_chars(head.line).take(head.column).collect();
        if self.indent.tabs || leading.len() < 2 || !leading.chars().all(|ch| ch == ' ') {
            return false;
        }
        let unit = self.indent.width.max(1);
        let step = match leading.len() % unit {
            0 => unit,
            rest => rest,
        };
        self.replace(Position::new(head.line, head.column - step)..head, "");
        true
    }
}
