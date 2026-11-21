//! The project's files under the pointer and the keyboard: rows dragged into another folder, the keys that cut, copy, paste, rename and delete them, and files of this machine let go on them.

use pm_components::target::Target;
use pm_components::tree_actions::{TreeShortcut, TreeTarget};
use rustle::{Key, KeyEvent};

use super::App;
use crate::keys::{self, Clip};

impl App {
    /// Follows the pointer to `at` while the row of the project's files `id` is carried,
    /// over the row `over` when it is over one: the folder there is where the row would
    /// land. Nothing follows once the drag was given up.
    pub(super) fn drag_tree(&mut self, id: Target, at: (f32, f32), over: Option<TreeTarget>) {
        if !self.model.read(|screen| screen.tree_dragging()) {
            return;
        }
        let Some(from) = self.model.read(|screen| screen.tree_row_of(id)) else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.tree_dragged(from, over, at))
        });
    }

    /// Ends a drag of rows of the project's files, moving them into the folder they were let go on, or copying them there while the key that copies is held in `modifiers`, and says whether there was a drag.
    pub(super) fn drop_tree(&mut self, modifiers: rustle::Modifiers) -> bool {
        if !self.model.read(|screen| screen.tree_dragging()) {
            return false;
        }
        let copy = if cfg!(target_os = "macos") {
            modifiers.alt
        } else {
            modifiers.ctrl
        };
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.tree_dropped(copy))
        });
        self.deliver();
        true
    }

    /// Carries out a key aimed at the project's files, and says whether it was one: Escape gives up a drag, the clipboard's keys cut, copy and paste, F2 renames and Delete deletes.
    pub(super) fn tree_key(&mut self, event: &KeyEvent) -> bool {
        let command = event.modifiers.ctrl || event.modifiers.meta;
        let shortcut = match (keys::clip(event), &event.key) {
            (_, Key::Escape) if self.model.read(|screen| screen.tree_dragging()) => {
                self.model.update(|state| {
                    *state = std::mem::take(state).in_workspace(|state| state.without_tree_drag())
                });
                return true;
            }
            (Some(Clip::Cut), _) => TreeShortcut::Cut,
            (Some(Clip::Copy), _) => TreeShortcut::Copy,
            (Some(Clip::Paste), _) => {
                if !self.model.read(|screen| screen.tree_holding()) {
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
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.tree_shortcut(shortcut))
        });
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
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.tree_brought_in(paths, over))
        });
        self.deliver();
    }

    /// Shows where files held over the window at `at`, over the row `over` of the project's files when they are over one, would land; they are held nowhere when they left the window.
    pub(super) fn files_hovered(&mut self, at: Option<(f32, f32)>, over: Option<TreeTarget>) {
        let over = over.filter(|_| at.is_some());
        self.model.update(|state| {
            let taken = std::mem::take(state);
            *state = match at {
                Some(at) => taken.in_workspace(|state| state.tree_hovered(over, at)),
                None => taken.in_workspace(|state| state.without_tree_drag()),
            };
        });
    }

    /// Takes in the files of this machine at `paths` let go over the window, over the row `over` of the project's files when they were let go on one: there they are brought into the folder of the row, and anywhere else they are attached to the prompt.
    pub(super) fn files_dropped(&mut self, paths: Vec<String>, over: Option<TreeTarget>) {
        match over {
            Some(over) => self.files_dropped_on(paths, Some(over)),
            None => {
                self.model.update(|state| {
                    *state = std::mem::take(state).in_workspace(|state| state.without_tree_drag())
                });
                self.attach_paths(paths);
            }
        }
    }
}
