//! What the language's scopes change where the caret is: the comment marks, whether Emmet expands, and the tag a `>` typed closes.

use std::ops::Range;

use pm_text::Edit;
use pm_treesitter::{Scope, Syntax};

use crate::document::Document;

impl Document {
    /// The scope of the language the byte `offset` of the text is in, when it is in one.
    fn scope_at(&self, offset: usize) -> Option<&Scope> {
        self.syntax.as_ref()?.scope_at(offset)
    }

    /// The name of the scope of the language the caret is in, when it is in one.
    pub fn scope_name(&self) -> Option<String> {
        self.scope_at(self.field.caret())
            .map(|scope| scope.name.clone())
    }

    /// What starts a comment that runs to the end of the line at the byte `offset`: the scope's when it names one, the language's otherwise; none when there are no line comments there.
    pub(crate) fn line_comment_at(&self, offset: usize) -> Option<String> {
        let own = self
            .scope_at(offset)
            .and_then(|scope| scope.line_comment.clone());
        own.or_else(|| self.editing.line_comment.clone())
            .filter(|prefix| !prefix.is_empty())
    }

    /// What opens and closes a comment at the byte `offset`, when the scope it is in has them.
    fn block_comment_at(&self, offset: usize) -> Option<(String, String)> {
        self.scope_at(offset)?.block_comment.clone()
    }

    /// Whether an Emmet abbreviation written over the bytes `range` is expanded: always in a language whose scopes do not speak of Emmet, and in one whose do only where the scope the abbreviation starts in turns it on. The abbreviation is what is being typed and often is not valid in the language, so the scope is found in the text with it masked.
    pub(crate) fn emmet_at(&self, range: Range<usize>) -> bool {
        let speaks = self
            .editing
            .scopes
            .iter()
            .any(|scope| scope.emmet.is_some());
        if !speaks {
            return true;
        }
        let Some(syntax) = &self.syntax else {
            return false;
        };
        let text = self.field.text();
        let masked = format!(
            "{}{}{}",
            &text[..range.start],
            "x".repeat(range.len()),
            &text[range.end..]
        );
        Syntax::parse(syntax.grammar().clone(), &masked)
            .and_then(|parsed| parsed.scope_at(range.start).map(|scope| scope.emmet))
            == Some(Some(true))
    }

    /// The edit that types `>` at the caret and writes the closing tag after it, when the `>` ends an opening tag that nothing closes yet; the caret is left between the two.
    pub(crate) fn closing_tag_edit(&self) -> Option<Edit> {
        let caret = self.field.caret();
        let closing = self
            .syntax
            .as_ref()?
            .closing_tag(self.field.text(), caret)?;
        let inside = caret + 1;
        Some(Edit::Replace(
            caret..caret,
            format!(">{closing}"),
            (inside, inside),
        ))
    }

    /// The edit that wraps the lines `lines`, with the first holding something at `first`, in the block comment the scope there has, or takes it away when they are in one.
    pub(crate) fn toggle_block_comment_edit(
        &self,
        lines: &[(Range<usize>, usize)],
        first: usize,
    ) -> Option<Edit> {
        let (open, close) = self.block_comment_at(first)?;
        let text = self.field.text();
        let (last, _) = lines.last()?;
        let end = last.start + text[last.clone()].trim_end().len();
        let held = &text[first..end];
        let wrapped = held.len() >= open.len() + close.len()
            && held.starts_with(&open)
            && held.ends_with(&close);
        let splices = if wrapped {
            let inside = &held[open.len()..held.len() - close.len()];
            let before = usize::from(inside.starts_with(' '));
            let after = usize::from(inside.len() > before && inside.ends_with(' '));
            vec![
                (first, open.len() + before, String::new()),
                (
                    end - close.len() - after,
                    close.len() + after,
                    String::new(),
                ),
            ]
        } else {
            vec![
                (first, 0, format!("{open} ")),
                (end, 0, format!(" {close}")),
            ]
        };
        self.spliced(splices)
    }
}
