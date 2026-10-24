//! The keys that work on whole lines and on the selection of the file shown: commenting, indenting, moving, duplicating and deleting lines, growing and shrinking the selection along the syntax tree, going to the next problem and to a line by its number.

use pm_editor::Document;
use pm_input::FocusId;
use pm_text::{Edit, Field};

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
            LineEdit::Fold | LineEdit::Unfold | LineEdit::UnfoldAll
        ) {
            if let Some(document) = self.shown_text() {
                match what {
                    LineEdit::Fold => {
                        document.fold_at_caret();
                    }
                    LineEdit::Unfold => {
                        document.unfold_at_caret();
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
            _ => self.line_edit(what).into_iter().collect(),
        };
        let read_only = self
            .shown_document()
            .is_none_or(|document| document.field.is_read_only());
        let moves_only = matches!(
            what,
            LineEdit::Grow
                | LineEdit::Shrink
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
            LineEdit::SelectAllOccurrences
            | LineEdit::Fold
            | LineEdit::Unfold
            | LineEdit::UnfoldAll => None,
        })
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
