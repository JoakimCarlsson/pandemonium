//! The keys that work on whole lines and on the selection of the file shown: commenting, indenting, moving, duplicating and deleting lines, growing and shrinking the selection along the syntax tree, going to the next problem and to a line by its number.

use pm_editor::{Case, Document, LineOrder};
use pm_input::FocusId;
use pm_text::{Edit, Field, Motion};

use crate::state::WorkspaceState;

/// The field the number of the line to go to is typed in.
pub const GO_TO_LINE: FocusId = FocusId(7_401_910);

/// Hands in the number of the line to go to.
pub const GO_TO_LINE_SUBMIT: FocusId = FocusId(7_401_911);

/// The box the number of the line to go to is typed in, which keeps a press on it from counting as outside.
pub const GO_TO_LINE_BOX: FocusId = FocusId(7_401_912);

/// What is done to the lines or the selection of the file shown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEdit {
    /// Comments the lines out, or back in.
    ToggleComment,
    /// Indents the lines one level.
    Indent,
    /// Takes a level of indentation off the lines.
    Outdent,
    /// Moves the lines up past the line above.
    MoveUp,
    /// Moves the lines down past the line below.
    MoveDown,
    /// Writes the lines again above, the selection staying up.
    DuplicateUp,
    /// Writes the lines again below, the selection going down with the copy.
    DuplicateDown,
    /// Deletes the lines.
    Delete,
    /// Grows the selection to the piece of syntax around it.
    Grow,
    /// Shrinks the selection back to what it was before it grew.
    Shrink,
    /// Selects the function, class or comment around the caret as the language's text objects say, what is inside it when the flag is set.
    SelectObject(pm_treesitter::TextObject, bool),
    /// Goes to the next stretch the language server finds wrong.
    NextProblem,
    /// Goes to the stretch before that the language server finds wrong.
    PreviousProblem,
    /// Backspace: an empty pair deleted whole, a level of indentation at once, or one character.
    Backspace,
    /// Selects the next place the selection comes again as well, or the word at the caret.
    AddNextOccurrence,
    /// Selects every place the selection, or the word at the caret, comes.
    SelectAllOccurrences,
    /// Adds a caret on the line above.
    AddCaretAbove,
    /// Adds a caret on the line below.
    AddCaretBelow,
    /// Keeps only the selection the caret is in.
    OnlyCaret,
    /// Folds away the block around the caret.
    Fold,
    /// Unfolds the block the caret's line stands for.
    Unfold,
    /// Folds every block that is not inside another.
    FoldAll,
    /// Takes back the last move of the carets.
    CursorUndo,
    /// Makes again the last move of the carets that was taken back.
    CursorRedo,
    /// Unfolds every block.
    UnfoldAll,
    /// Selects the line, or the next one as well.
    SelectLine,
    /// Joins the line with the next.
    JoinLines,
    /// Goes to the bracket pairing with the one at the caret.
    MatchingBracket,
    /// Starts a new line below.
    LineBelow,
    /// Starts a new line above.
    LineAbove,
    /// Goes to where the text of the line starts, or to its very start; the selection grows when the flag is set.
    Home(bool),
    /// Writes the selections, or the word at the caret, in a case.
    Case(Case),
    /// Puts the selected lines, or every line, in another order.
    OrderLines(LineOrder),
    /// Swaps the characters around the caret.
    Transpose,
    /// Takes the spaces and tabs off the ends of the lines.
    TrimTrailing,
    /// Deletes from the caret to the end of its line.
    DeleteToLineEnd,
    /// Deletes from the caret to the start of its line.
    DeleteToLineStart,
    /// Wraps the lines in a block comment, or takes it away.
    ToggleBlockComment,
    /// Puts a caret at the end of each selected line.
    SplitIntoLines,
    /// Grows the rectangular selection by this many lines down and columns right, which may be negative.
    Column(isize, isize),
    /// Selects the rectangle with corners at these two bytes of the text, as dragging the pointer over it does.
    ColumnDrag(usize, usize),
}

/// The box for going to a line by its number.
#[derive(Clone, Debug, PartialEq)]
pub struct GoingTo {
    /// The number being typed.
    pub field: Field,
    /// Where it hangs, in logical pixels of the window.
    pub at: (f32, f32),
}

impl WorkspaceState {
    /// The file shown, to change it, when it is text.
    pub(crate) fn shown_text(&mut self) -> Option<&mut Document> {
        let index = self.git.shown?;
        match &mut self.git.viewers.get_mut(index)?.contents {
            crate::open_files::Contents::Text(document) => Some(document),
            _ => None,
        }
    }

    /// Does `what` to the lines or the selection of the file shown.
    pub fn lines_edited(mut self, what: LineEdit) -> Self {
        if matches!(
            what,
            LineEdit::Fold
                | LineEdit::Unfold
                | LineEdit::FoldAll
                | LineEdit::UnfoldAll
                | LineEdit::CursorUndo
                | LineEdit::CursorRedo
        ) {
            if let Some(document) = self.shown_text() {
                match what {
                    LineEdit::Fold => {
                        document.fold_at_caret();
                    }
                    LineEdit::Unfold => {
                        document.unfold_at_caret();
                    }
                    LineEdit::FoldAll => document.fold_all(),
                    LineEdit::CursorUndo => {
                        document.cursor_undo();
                    }
                    LineEdit::CursorRedo => {
                        document.cursor_redo();
                    }
                    _ => document.unfold_all(),
                }
            }
            return self;
        }
        let several = self
            .shown_document()
            .is_some_and(|document| document.field.has_others());
        let edits: Vec<Edit> = match what {
            LineEdit::SelectAllOccurrences => self
                .shown_document()
                .map(Document::select_all_occurrences_edits)
                .unwrap_or_default(),
            LineEdit::Backspace if several => vec![Edit::Backspace],
            LineEdit::Home(extend) if several => {
                vec![Edit::Move(pm_text::Motion::LineStart, extend)]
            }
            LineEdit::Case(case) => self
                .shown_document()
                .map(|document| document.case_edits(case))
                .unwrap_or_default(),
            LineEdit::OrderLines(order) => self
                .shown_document()
                .map(|document| document.order_lines_edits(order))
                .unwrap_or_default(),
            LineEdit::SplitIntoLines => self
                .shown_document()
                .map(Document::split_into_lines_edits)
                .unwrap_or_default(),
            LineEdit::Column(down, right) => self
                .shown_text()
                .map(|document| document.column_select_edits(down, right))
                .unwrap_or_default(),
            LineEdit::ColumnDrag(from, to) => self
                .shown_document()
                .map(|document| document.column_drag_edits(from, to))
                .unwrap_or_default(),
            LineEdit::DeleteToLineEnd => vec![Edit::Move(Motion::LineEnd, true), Edit::Delete],
            LineEdit::DeleteToLineStart => {
                vec![Edit::Move(Motion::LineStart, true), Edit::Backspace]
            }
            _ => self.line_edit(what).into_iter().collect(),
        };
        let read_only = self
            .shown_document()
            .is_none_or(|document| document.field.is_read_only());
        let moves_only = matches!(
            what,
            LineEdit::Grow
                | LineEdit::Shrink
                | LineEdit::SelectObject(..)
                | LineEdit::NextProblem
                | LineEdit::PreviousProblem
                | LineEdit::AddNextOccurrence
                | LineEdit::SelectAllOccurrences
                | LineEdit::AddCaretAbove
                | LineEdit::AddCaretBelow
                | LineEdit::OnlyCaret
                | LineEdit::SelectLine
                | LineEdit::MatchingBracket
                | LineEdit::Home(_)
                | LineEdit::SplitIntoLines
                | LineEdit::Column(..)
                | LineEdit::ColumnDrag(..)
        );
        if moves_only || !read_only {
            for edit in edits {
                self.file_edited(edit);
            }
        }
        self
    }

    /// The edit `what` makes to the file shown, when it makes one.
    fn line_edit(&mut self, what: LineEdit) -> Option<Edit> {
        self.shown_text().and_then(|document| match what {
            LineEdit::ToggleComment => document.toggle_comment_edit(),
            LineEdit::Indent => document.indent_lines_edit(false),
            LineEdit::Outdent => document.indent_lines_edit(true),
            LineEdit::MoveUp => document.move_lines_edit(false),
            LineEdit::MoveDown => document.move_lines_edit(true),
            LineEdit::DuplicateUp => document.duplicate_lines_edit(false),
            LineEdit::DuplicateDown => document.duplicate_lines_edit(true),
            LineEdit::Delete => document.delete_lines_edit(),
            LineEdit::Grow => document.grow_selection_edit(),
            LineEdit::Shrink => document.shrink_selection_edit(),
            LineEdit::SelectObject(object, inside) => document.select_object_edit(object, inside),
            LineEdit::NextProblem => document.next_diagnostic_edit(false),
            LineEdit::PreviousProblem => document.next_diagnostic_edit(true),
            LineEdit::Backspace => Some(document.backspace_edit().unwrap_or(Edit::Backspace)),
            LineEdit::AddNextOccurrence => document.add_next_occurrence_edit(),
            LineEdit::AddCaretAbove => document.add_caret_line_edit(false),
            LineEdit::AddCaretBelow => document.add_caret_line_edit(true),
            LineEdit::OnlyCaret => Some(Edit::OnlySelection),
            LineEdit::SelectLine => Some(document.select_line_edit()),
            LineEdit::JoinLines => document.join_lines_edit(),
            LineEdit::MatchingBracket => document.matching_bracket_edit(),
            LineEdit::LineBelow => Some(document.open_line_edit(false)),
            LineEdit::LineAbove => Some(document.open_line_edit(true)),
            LineEdit::Home(extend) => Some(document.home_edit(extend)),
            LineEdit::Transpose => document.transpose_edit(),
            LineEdit::TrimTrailing => document.tidy_edit(true, false),
            LineEdit::ToggleBlockComment => document.toggle_block_comment_lines_edit(),
            LineEdit::Case(_)
            | LineEdit::OrderLines(_)
            | LineEdit::DeleteToLineEnd
            | LineEdit::DeleteToLineStart
            | LineEdit::SplitIntoLines
            | LineEdit::Column(..)
            | LineEdit::ColumnDrag(..)
            | LineEdit::SelectAllOccurrences
            | LineEdit::Fold
            | LineEdit::Unfold
            | LineEdit::FoldAll
            | LineEdit::CursorUndo
            | LineEdit::CursorRedo
            | LineEdit::UnfoldAll => None,
        })
    }

    /// What copying in the file shown puts on the clipboard when the selection holds nothing: the lines the carets are on, the indentation the text was copied at, and whether it is whole lines; or the indentation of the selection when it holds something.
    pub fn editor_copied(&self) -> Option<(Option<String>, String)> {
        let document = self.shown_document()?;
        Some((document.copied_lines(), document.copied_indentation()))
    }

    /// The edit that pastes `text` into the file shown, when it was copied from the file at `lines` whole lines or at `indentation`, and the paste is better made at the place it goes to than as it is.
    pub fn editor_paste_edit(
        &self,
        text: &str,
        whole_lines: bool,
        indentation: &str,
    ) -> Option<Edit> {
        let document = self.shown_document()?;
        if whole_lines {
            document.paste_lines_edit(text)
        } else {
            document.paste_reindented_edit(text, indentation)
        }
    }

    /// Whether the selection of the file shown runs over more than one line.
    pub fn selection_spans_lines(&self) -> bool {
        self.shown_document().is_some_and(|document| {
            document
                .field
                .selection()
                .is_some_and(|range| document.field.text()[range].contains('\n'))
        })
    }

    /// Opens the box for going to a line by its number, at `at`.
    pub fn go_to_line_requested(mut self, at: (f32, f32)) -> Self {
        if self.shown_document().is_some() {
            self.going_to = Some(GoingTo {
                field: Field::new(""),
                at,
            });
        }
        self
    }

    /// Whether the box for going to a line is open.
    pub fn going_to_open(&self) -> bool {
        self.going_to.is_some()
    }

    /// Closes the box for going to a line.
    pub fn go_to_line_closed(mut self) -> Self {
        self.going_to = None;
        self
    }

    /// Goes to the line whose number is typed in the box, and closes it; a number that is not one leaves it open.
    pub fn go_to_line_submitted(mut self) -> Self {
        let Some(number) = self
            .going_to
            .as_ref()
            .and_then(|going| going.field.text().trim().parse::<usize>().ok())
        else {
            return self;
        };
        self.going_to = None;
        self.remember_spot();
        let edit = self
            .shown_document()
            .map(|document| document.go_to_line_edit(number));
        if let Some(edit) = edit {
            self.file_edited(edit);
        }
        self
    }
}
