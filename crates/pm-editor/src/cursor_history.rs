//! Taking back the last move of the carets and selections without touching the text, and making it again.

use pm_text::Edit;

use crate::document::Document;

/// How many moves of the carets are remembered.
const REMEMBERED: usize = 100;

/// Where the carets and selections were at one moment, as the end that does not move and the end that does of each.
pub(crate) type Carets = Vec<(usize, usize)>;

impl Document {
    /// Remembers `before`, where the carets were before a move that changed nothing else, and forgets the moves that were taken back.
    pub(crate) fn note_cursor_move(&mut self, before: Carets) {
        if self.cursor_past.last() != Some(&before) {
            self.cursor_past.push(before);
        }
        if self.cursor_past.len() > REMEMBERED {
            self.cursor_past.remove(0);
        }
        self.cursor_future.clear();
    }

    /// Forgets the moves of the carets, which a change of the text leaves pointing at the wrong places.
    pub(crate) fn forget_cursor_moves(&mut self) {
        self.cursor_past.clear();
        self.cursor_future.clear();
    }

    /// Puts the carets and selections at `carets`, without it counting as a move.
    fn put_carets(&mut self, carets: &Carets) {
        let length = self.field.text().len();
        let mut held = carets
            .iter()
            .map(|(anchor, caret)| (*anchor.min(&length), *caret.min(&length)));
        if let Some((anchor, caret)) = held.next() {
            self.field.apply(Edit::Select(anchor, caret));
        }
        for (anchor, caret) in held {
            self.field.apply(Edit::AddSelection(anchor, caret));
        }
    }

    /// Takes back the last move of the carets or selections; says whether there was one.
    pub fn cursor_undo(&mut self) -> bool {
        let Some(previous) = self.cursor_past.pop() else {
            return false;
        };
        self.cursor_future.push(self.field.selections());
        self.put_carets(&previous);
        true
    }

    /// Makes again the last move of the carets or selections that was taken back; says whether there was one.
    pub fn cursor_redo(&mut self) -> bool {
        let Some(next) = self.cursor_future.pop() else {
            return false;
        };
        self.cursor_past.push(self.field.selections());
        self.put_carets(&next);
        true
    }
}
