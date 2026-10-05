//! One pane layout for each project.
//!
//! A project's window is its own: the way it is divided, which tools sit
//! where and how wide each column is belong to that project and to no other.
//! The tree being drawn is always the active project's; the others wait here
//! until their project is pointed at again. A project opened for the first
//! time starts from the default division, and a session is cut into the
//! division of the project it belongs to.

use std::collections::{BTreeMap, BTreeSet};

use pm_core::{ProjectId, Scope};

use crate::app::App;
use crate::app::placement::Recent;
use crate::panes::{Item, PaneTree};

/// A project's pane tree while another project is the one on screen.
pub(super) struct Shelved {
    /// How the project's window is divided and what is open in it.
    panes: PaneTree,
    /// The pane last used for each role in that tree.
    recent: Recent,
}

/// The pane trees of every project that is not on screen.
pub(super) type Shelf = BTreeMap<ProjectId, Shelved>;

impl App {
    /// Puts the active project's pane tree on screen, shelving the one that was.
    ///
    /// This is the one place trees change hands: whatever made another
    /// project active, the next thing the window does finds that project's
    /// own panes. A project with no tree yet gets the default division.
    pub(super) fn sync_layout(&mut self) {
        let active = self.open.active().map(pm_core::Project::id);
        if active == self.layout_of {
            return;
        }
        if let Some(owner) = self.layout_of.take() {
            self.shelve_layout(owner);
        }
        self.layout_of = active;
        self.drag = None;
        self.closing = None;
        self.menu = None;
        let Some(project) = active else {
            return;
        };
        match self.shelf.remove(&project) {
            Some(shelved) => {
                self.panes = shelved.panes;
                self.recent = shelved.recent;
            }
            None => self.start_layout(project),
        }
    }

    /// Moves the tree on screen to the shelf, under `owner`.
    fn shelve_layout(&mut self, owner: ProjectId) {
        let panes = std::mem::take(&mut self.panes);
        let recent = std::mem::take(&mut self.recent);
        self.shelf.insert(owner, Shelved { panes, recent });
    }

    /// Opens every project's saved layout, then puts the active project's on screen.
    pub(super) fn restore_layouts(&mut self, saved: &[crate::panes::SavedLayout]) {
        for layout in saved {
            let owner = self
                .open
                .iter()
                .find(|project| project.root() == layout.project)
                .map(pm_core::Project::id);
            if let Some(owner) = owner {
                self.restore_panes(&layout.panes, owner);
            }
        }
        self.layout_of = None;
        self.sync_layout();
    }

    /// Shelves the tree on screen under `project`, which the tree was restored for.
    pub(super) fn shelve_restored(&mut self, project: ProjectId) {
        self.shelve_layout(project);
    }

    /// Replaces the tree on screen with the default division for `project`.
    fn start_layout(&mut self, project: ProjectId) {
        let scope = Scope::checkout(project);
        let (layout, focus) = self.default_arrangement(Some(scope), Vec::new(), &BTreeSet::new());
        let mut panes = PaneTree::default();
        panes.arrange(&layout, focus, Some(scope));
        self.panes = panes;
        self.recent = Recent::new();
        let documents = self.panes.panes().get(focus).copied();
        if let Some(documents) = documents {
            self.recent.insert(crate::panes::Role::Editor, documents);
        }
    }

    /// Forgets the layout of a project the window no longer holds.
    pub(super) fn forget_layout(&mut self, project: ProjectId) {
        self.shelf.remove(&project);
        if self.layout_of == Some(project) {
            self.layout_of = None;
            self.panes = PaneTree::default();
            self.recent = Recent::new();
        }
    }

    /// Everything open in any pane of any project's window.
    pub(super) fn held_everywhere(&self) -> BTreeSet<Item> {
        let mut held = self.panes.held();
        for shelved in self.shelf.values() {
            held.extend(shelved.panes.held());
        }
        held
    }

    /// Every project's layout, as it is written down.
    pub(super) fn saved_layouts(&self) -> Vec<crate::panes::SavedLayout> {
        let mut layouts = Vec::new();
        for project in self.open.iter() {
            let panes = if self.layout_of == Some(project.id()) {
                &self.panes
            } else if let Some(shelved) = self.shelf.get(&project.id()) {
                &shelved.panes
            } else {
                continue;
            };
            let scope = match self.layout_of == Some(project.id()) {
                true => self.scope(),
                false => Some(Scope::checkout(project.id())),
            };
            layouts.push(crate::panes::SavedLayout {
                project: project.root().to_path_buf(),
                panes: self.saved_panes(panes, scope),
            });
        }
        layouts
    }
}
