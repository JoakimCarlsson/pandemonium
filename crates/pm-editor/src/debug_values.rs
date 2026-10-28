//! The names of a file whose value is shown while the program is paused, found by the language's own debugger query where its language server does not say.

use pm_protocol::{InlineValue, InlineValueKind, Span};

use crate::document::Document;

impl Document {
    /// The names to show the value of when the program is paused on the line `line`: those the language's debugger query marks from the start of the scope that line is in to the end of the line, each a variable to look up; none when the language marks none.
    pub fn debug_values(&self, line: usize) -> Vec<InlineValue> {
        let Some(syntax) = &self.syntax else {
            return Vec::new();
        };
        let text = self.field.text();
        let held = self.line(self.line_of(text.len()).min(line));
        let start = syntax
            .debug_scope(text, held.start)
            .map_or(0, |scope| scope.start);
        syntax
            .debug_variables(text, start..held.end)
            .into_iter()
            .map(|(range, name)| InlineValue {
                span: Span::new(self.position_of(range.start), self.position_of(range.end)),
                kind: InlineValueKind::Variable(Some(name)),
            })
            .collect()
    }
}
