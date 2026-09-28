//! What the window does with files carried onto it from outside.
//!
//! Over the file tree they land the way rows carried inside it do: in the
//! directory under the pointer, or the one holding the file under it, and
//! the tree marks that directory while they are over it. They are always
//! copied in; what was carried stays where it was.

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

    /// Follows files carried in from outside, and copies them into the tree
    /// when they are let go over it.
    pub(super) fn arrive(&mut self, arrival: Arrival) {
        match arrival {
            Arrival::Hovering(at) => self.arriving = self.arrival_directory(at),
            Arrival::Left => self.arriving = None,
            Arrival::Dropped { at, paths } => {
                let directory = self.arrival_directory(at).or(self.arriving.take());
                self.arriving = None;
                if let Some(directory) = directory {
                    self.move_entries(&paths, &directory, true);
                }
            }
        }
        self.request_redraw();
    }

    /// The directory files carried in at `at` would land in.
    ///
    /// Where the platform does not say where they are, the pointer's last
    /// place stands in for it, and failing that the worktree the tree shows.
    fn arrival_directory(&self, at: Option<Point>) -> Option<PathBuf> {
        match at.or(self.pointer) {
            Some(point) => self.directory_under(point),
            None => self
                .tree_showing()
                .then(|| self.scope().and_then(|scope| self.files.get(&scope)))
                .flatten()
                .map(|tree| tree.root().to_path_buf()),
        }
    }
}
