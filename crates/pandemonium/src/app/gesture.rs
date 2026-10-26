//! The places the pointer selects text, each as a [`Surface`] for the one
//! press-and-drag gesture: a field, the conversation's text, and the shell.

use pm_input::FocusId;
use pm_pages::Screen;
use pm_state::State;
use pm_text::{Edit, Reach, Surface};
use pm_ui::{Output, TextPoint, TextSelection};

use super::App;

/// The text of the field `id`, by byte offset into what it holds.
pub(super) struct FieldText<'a> {
    /// The app that holds the field.
    pub(super) app: &'a mut App,
    /// The field.
    pub(super) id: FocusId,
}

impl Surface for FieldText<'_> {
    type Pos = usize;

    /// The word or line around the byte `at` of the field.
    fn reach_around(&self, reach: Reach, at: usize) -> (usize, usize) {
        self.app
            .model
            .read(|screen| {
                screen
                    .field(self.id)
                    .map(|field| field.reach_around(reach, at))
            })
            .unwrap_or((at, at))
    }

    /// Puts the caret of the field at the byte `at`.
    fn place(&mut self, at: usize) {
        self.app.edit_field(self.id, Edit::Select(at, at));
    }

    /// Selects the field from the byte `anchor` to the byte `head`.
    fn select(&mut self, anchor: usize, head: usize) {
        self.app.edit_field(self.id, Edit::Select(anchor, head));
    }
}

/// The conversation's selectable text `id`, by place in its drawn lines.
pub(super) struct ChatText<'a> {
    /// What was drawn.
    pub(super) output: &'a Output,
    /// The selection the app holds.
    pub(super) selection: &'a mut Option<TextSelection>,
    /// The text.
    pub(super) id: FocusId,
}

impl Surface for ChatText<'_> {
    type Pos = TextPoint;

    /// The word or paragraph around the place `at`.
    fn reach_around(&self, reach: Reach, at: TextPoint) -> (TextPoint, TextPoint) {
        self.output.unit_around(self.id, at, reach)
    }

    /// Starts an empty selection at `at`.
    fn place(&mut self, at: TextPoint) {
        self.select(at, at);
    }

    /// Selects from the place `anchor` to the place `head`.
    fn select(&mut self, anchor: TextPoint, head: TextPoint) {
        *self.selection = Some(TextSelection {
            id: self.id,
            anchor,
            head,
        });
    }
}

/// The window of the shell showing, by line counted from the oldest kept and column.
pub(super) struct TerminalWindow<'a> {
    /// The screen the shell is on.
    pub(super) model: &'a State<Screen>,
}

impl Surface for TerminalWindow<'_> {
    type Pos = (usize, usize);

    /// The word or line around the cell `at`.
    fn reach_around(&self, reach: Reach, at: (usize, usize)) -> ((usize, usize), (usize, usize)) {
        self.model
            .read(|screen| screen.terminal_reach(reach, at))
            .unwrap_or((at, at))
    }

    /// Unmarks the text.
    fn place(&mut self, _at: (usize, usize)) {
        self.model
            .update(|screen| screen.clear_terminal_selection());
    }

    /// Marks the text from the cell `anchor` to the cell `head`, both included.
    fn select(&mut self, anchor: (usize, usize), head: (usize, usize)) {
        self.model
            .update(|screen| screen.select_terminal_span(anchor, head));
    }
}
