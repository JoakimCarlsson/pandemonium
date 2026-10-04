//! What the window does with files carried onto it from outside.
//!
//! Over the file tree they land the way rows carried inside it do: in the
//! directory under the pointer, or the one holding the file under it, and
//! the tree marks that directory while they are over it. They are always
//! copied in; what was carried stays where it was.
//! Over the rest of the window, files open in the pane under the pointer.

use std::path::PathBuf;

use pm_gfx::Point;

use crate::app::App;
use crate::arrival::Arrival;

impl App {
    /// Takes in every arrival the platform's data device has seen.
    pub(super) fn take_arrivals(&mut self) {
        let arrivals = self
            .arrivals
            .lock()
            .map(|mut arrivals| std::mem::take(&mut *arrivals))
            .unwrap_or_default();
        for arrival in arrivals {
            self.arrive(arrival);
        }
    }

    /// Opens files carried onto panes, or copies them into the file tree.
    pub(super) fn arrive(&mut self, arrival: Arrival) {
        match arrival {
            Arrival::Hovering(at) => self.arriving = self.arrival_directory(at),
            Arrival::Left => self.arriving = None,
            Arrival::Dropped { at, paths } => {
                let directory = self.arrival_directory(at);
                self.arriving = None;
                if let Some(directory) = directory {
                    self.move_entries(&paths, &directory, true);
                } else {
                    self.open_arrivals(at, paths);
                }
            }
        }
        self.request_redraw();
    }

    /// The directory files carried in at `at` would land in.
    ///
    /// Where the platform does not say where they are, the pointer's last
    /// place stands in for it.
    fn arrival_directory(&self, at: Option<Point>) -> Option<PathBuf> {
        at.or(self.pointer)
            .and_then(|point| self.directory_under(point))
    }

    /// Opens dropped files in their worktrees or as window-wide loose files.
    fn open_arrivals(&mut self, at: Option<Point>, paths: Vec<PathBuf>) {
        let pane = at
            .or(self.pointer)
            .and_then(|point| self.geometry.pane_at(point))
            .unwrap_or_else(|| self.panes.focus());
        for path in paths {
            let path = match path.canonicalize() {
                Ok(path) => path,
                Err(trouble) => {
                    self.say_trouble("The dropped path could not be opened", &trouble);
                    continue;
                }
            };
            if !path.is_file() {
                continue;
            }
            if self.worktree_holding(&path).is_some() {
                self.open_tree_file(&path, pane, false);
            } else if let Some(file) = self.editor.open_loose(&path) {
                self.show_file(pane, file, false);
            } else {
                self.notices
                    .trouble(format!("Could not read {}", path.display()), None);
            }
        }
    }
}
