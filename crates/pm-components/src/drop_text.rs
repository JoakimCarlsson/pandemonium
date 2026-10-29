//! Moving or copying selected text of the file shown by dragging it: what the window does as the button goes down on the selection, as the pointer moves over the text, and as the button comes up.

use crate::state::WorkspaceState;

impl WorkspaceState {
    /// Picks up the selection of the file shown, pressed on at the byte `byte`, when dragging text is on and there is a selection there to pick up; says whether it did.
    pub fn editor_drag_started(&mut self, byte: usize) -> bool {
        let allowed = self.editor.drag_and_drop;
        allowed
            && self
                .shown_text()
                .is_some_and(|document| document.drag_started(byte))
    }

    /// Notes that the text being dragged is over the byte `byte` of the file shown.
    pub fn editor_drag_over(&mut self, byte: usize) {
        if let Some(document) = self.shown_text() {
            document.drag_over(byte);
        }
    }

    /// Puts down the text being dragged where it is over, as one change: moved, or copied when `copy`.
    pub fn editor_dropped(&mut self, copy: bool) {
        let edit = self
            .shown_text()
            .and_then(|document| document.drag_dropped(copy));
        if let Some(edit) = edit {
            self.file_edited(edit);
        }
    }

    /// Lets go of the text being dragged without moving it.
    pub fn editor_drag_cancelled(&mut self) {
        if let Some(document) = self.shown_text() {
            document.drag_cancelled();
        }
    }
}
