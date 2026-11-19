//! The project's files under the pointer and the keyboard: rows dragged into another folder, the keys that cut, copy, paste, rename and delete them, and files of this machine let go on them.

use pm_components::tree_actions::{TreeShortcut, TreeTarget};
use pm_pages::Screen;
use rustle::{Key, KeyEvent};

use super::App;
use crate::keys::{self, Clip};

impl App {
    /// The row of the project's files at `at`, or their root when it is the space under them.
    fn tree_row_at(&self, at: (f32, f32)) -> Option<TreeTarget> {
        let shown = self.model.read(|screen| screen.places().shown());
        shown
            .into_iter()
            .filter(|(_, rect)| {
                let rect = self.framed(*rect);
                at.0 >= rect.origin.x
                    && at.1 >= rect.origin.y
                    && at.0 < rect.origin.x + rect.size.width
                    && at.1 < rect.origin.y + rect.size.height
            })
            .find_map(|(id, _)| self.model.read(|screen| screen.tree_row_of(id)))
    }

    /// Follows the pointer to `at` while a row of the project's files is carried: the folder under the pointer is where the row would land.
    pub(super) fn drag_tree(&mut self, at: (f32, f32)) {
        let Some(from) = self.tree_press.clone() else {
            return;
        };
        let over = self.tree_row_at(at);
        self.model
            .update(|state| *state = std::mem::take(state).tree_dragged(from, over, at));
    }

    /// Ends a drag of rows of the project's files, moving them into the folder they were let go on, or copying them there while the key that copies is held in `modifiers`, and says whether there was a drag.
    pub(super) fn drop_tree(&mut self, modifiers: rustle::Modifiers) -> bool {
        self.tree_press = None;
        if !self.model.read(Screen::tree_dragging) {
            return false;
        }
        let copy = if cfg!(target_os = "macos") {
            modifiers.alt
        } else {
            modifiers.ctrl
        };
        self.model
            .update(|state| *state = std::mem::take(state).tree_dropped(copy));
        self.deliver();
        true
    }

    /// Carries out a key aimed at the project's files, and says whether it was one: Escape gives up a drag, the clipboard's keys cut, copy and paste, F2 renames and Delete deletes.
    pub(super) fn tree_key(&mut self, event: &KeyEvent) -> bool {
        let command = event.modifiers.ctrl || event.modifiers.meta;
        let shortcut = match (keys::clip(event), &event.key) {
            (_, Key::Escape) if self.model.read(Screen::tree_dragging) => {
                self.tree_press = None;
                self.model
                    .update(|state| *state = std::mem::take(state).without_tree_drag());
                return true;
            }
            (Some(Clip::Cut), _) => TreeShortcut::Cut,
            (Some(Clip::Copy), _) => TreeShortcut::Copy,
            (Some(Clip::Paste), _) => {
                if !self.model.read(Screen::tree_holding) {
                    self.paste_files_into_tree();
                    return true;
                }
                TreeShortcut::Paste
            }
            (None, Key::F(2)) => TreeShortcut::Rename,
            (None, Key::Delete) => TreeShortcut::Delete,
            (None, Key::Backspace) if cfg!(target_os = "macos") && command => TreeShortcut::Delete,
            _ => return false,
        };
        self.model
            .update(|state| *state = std::mem::take(state).tree_shortcut(shortcut));
        self.deliver();
        true
    }

    /// Brings the files another program put on the clipboard into the folder of the row last picked.
    fn paste_files_into_tree(&mut self) {
        let Some(clipboard) = self.clipboard() else {
            return;
        };
        let paths: Vec<String> = clipboard
            .files()
            .iter()
            .map(|file| file.to_string_lossy().into_owned())
            .collect();
        if !paths.is_empty() {
            self.files_dropped_on(paths, None);
        }
    }

    /// Brings the files of this machine at `paths` into the folder of `over`, or of the row last picked without it.
    fn files_dropped_on(&mut self, paths: Vec<String>, over: Option<TreeTarget>) {
        self.model
            .update(|state| *state = std::mem::take(state).tree_brought_in(paths, over));
        self.deliver();
    }

    /// Shows where files held over the window at `at` would land in the project's files; they are held nowhere when they left the window.
    #[cfg(target_os = "linux")]
    pub(super) fn files_hovered(&mut self, at: Option<(f32, f32)>) {
        let over = at.and_then(|at| self.tree_row_at(at));
        self.model.update(|state| {
            let taken = std::mem::take(state);
            *state = match at {
                Some(at) => taken.tree_hovered(over, at),
                None => taken.without_tree_drag(),
            };
        });
    }

    /// Takes in the files of this machine at `paths` let go over the window at `at`: over the project's files they are brought into the folder there, and anywhere else they are attached to the prompt.
    pub(super) fn files_dropped(&mut self, paths: Vec<String>, at: Option<(f32, f32)>) {
        match at.and_then(|at| self.tree_row_at(at)) {
            Some(over) => self.files_dropped_on(paths, Some(over)),
            None => {
                self.model
                    .update(|state| *state = std::mem::take(state).without_tree_drag());
                self.attach_paths(paths);
            }
        }
    }
}
