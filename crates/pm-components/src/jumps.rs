//! Going back to where the caret was before a jump to a definition, and forward again.

use pm_editor::Spot;

use crate::open_files::{Contents, jump};
use crate::state::{Tab, WorkspaceState};

impl WorkspaceState {
    /// Where the caret of the file shown is, when a file is shown.
    fn spot_here(&self) -> Option<Spot> {
        let session = self.review_index()?;
        let viewer = self.shown_viewer()?;
        let Contents::Text(document) = &viewer.contents else {
            return None;
        };
        Some(Spot {
            session,
            path: viewer.path.clone(),
            position: document.position_of(document.field.caret()),
        })
    }

    /// Remembers where the caret is before a jump, so going back returns there, and forgets the places gone back from.
    pub(crate) fn remember_spot(&mut self) {
        if let Some(spot) = self.spot_here() {
            self.jumps.leave(spot);
        }
    }

    /// Shows the file of `spot` with the caret at its place: the file open already, or the file of the project opened again; says whether it could.
    fn go_to(&mut self, spot: &Spot) -> bool {
        let row = self.editor.row_height();
        if self.review_index() != Some(spot.session) {
            return false;
        }
        if let Some(index) = self
            .git
            .viewers
            .iter()
            .position(|viewer| viewer.path == spot.path && !viewer.head)
        {
            self.git.shown = Some(index);
            self.tab = Tab::File;
            if let Some(viewer) = self.git.viewers.get_mut(index) {
                match &mut viewer.contents {
                    Contents::Text(document) => viewer.scroll = jump(document, spot.position, row),
                    _ => viewer.jump = Some(spot.position),
                }
            }
            return true;
        }
        if spot.path.starts_with('/') {
            return false;
        }
        self.open_viewer(spot.path.clone(), false);
        if let Some(viewer) = self
            .git
            .shown
            .and_then(|index| self.git.viewers.get_mut(index))
        {
            viewer.jump = Some(spot.position);
        }
        true
    }

    /// The state after going back to where the caret was before the last jump, skipping places that cannot be shown again.
    pub fn jumped_back(mut self) -> Self {
        let here = self.spot_here();
        while let Some(spot) = self.jumps.back.pop() {
            if self.go_to(&spot) {
                self.jumps.forward.extend(here);
                break;
            }
        }
        self
    }

    /// The state after going forward again to where the caret was before the last step back.
    pub fn jumped_forward(mut self) -> Self {
        let here = self.spot_here();
        while let Some(spot) = self.jumps.forward.pop() {
            if self.go_to(&spot) {
                self.jumps.back.extend(here);
                break;
            }
        }
        self
    }
}
