//! Where a tab goes when something asks for it to be opened.
//!
//! A pane has no fixed purpose, but what is open in it says what it is
//! for: files collect with files, conversations with conversations, shells
//! with shells. Opening anything goes through here, so a file never lands
//! in the chat and a conversation never lands among the files, whichever
//! way the layout was arranged.

use std::collections::BTreeMap;

use pm_core::Scope;

use crate::app::App;
use crate::panes::{Item, PaneId, Role, SplitDirection};

/// The panes the keyboard was last in for each role, so the next tab of a
/// role goes where the last one was looked at.
pub(super) type Recent = BTreeMap<Role, PaneId>;

impl App {
    /// Whether `pane` is at home to tabs of `role` while the window shows
    /// the current worktree.
    pub(super) fn serves(&self, pane: PaneId, role: Role) -> bool {
        let scope = self.scope();
        self.panes
            .pane(pane)
            .is_some_and(|pane| pane.serves(scope, role))
    }

    /// Whether `pane` holds a tab of `role` in the current worktree.
    fn holds(&self, pane: PaneId, role: Role) -> bool {
        let scope = self.scope();
        self.panes
            .pane(pane)
            .is_some_and(|pane| pane.holds(scope, role))
    }

    /// The pane a tab of `role` should open in, given the one asked for.
    ///
    /// The pane asked for stands when it serves the role. Otherwise the pane
    /// last used for the role, then any pane holding the role, then an empty
    /// one; and only when no pane will do is the window divided, beside the
    /// pane with the keyboard. A tool keeps to the pane it was asked for.
    pub(super) fn pane_for(&mut self, wanted: PaneId, role: Role) -> PaneId {
        if role == Role::Tool || self.serves(wanted, role) {
            return wanted;
        }
        let drawn = self.panes.drawn(self.scope());
        let recent = self
            .recent
            .get(&role)
            .copied()
            .filter(|pane| drawn.contains(pane) && self.serves(*pane, role));
        let found = recent
            .or_else(|| drawn.iter().copied().find(|pane| self.holds(*pane, role)))
            .or_else(|| drawn.iter().copied().find(|pane| self.serves(*pane, role)));
        if let Some(found) = found {
            return found;
        }
        let beside = self.panes.focus();
        let fresh = self
            .panes
            .split(beside, Self::room_for(role))
            .unwrap_or(beside);
        self.recent.insert(role, fresh);
        fresh
    }

    /// The side of a pane a new pane for `role` is made on.
    fn room_for(role: Role) -> SplitDirection {
        match role {
            Role::Terminal => SplitDirection::Down,
            Role::Editor | Role::Agent | Role::Tool => SplitDirection::Right,
        }
    }

    /// Remembers that `pane`, now showing `front`, is where its role was last used.
    pub(super) fn remember_role(&mut self, pane: PaneId, front: Option<Item>) {
        if let Some(role) = front.map(Item::role).filter(|role| *role != Role::Tool) {
            self.recent.insert(role, pane);
        }
    }

    /// Whether `pane` is holding documents, before its tabs change.
    pub(super) fn holds_documents(&self, pane: PaneId) -> bool {
        self.panes
            .pane(pane)
            .is_some_and(|pane| pane.items().any(|item| item.role() == Role::Editor))
    }

    /// Closes `pane` if its tabs are gone, unless it was the window's last
    /// place for documents.
    ///
    /// Closing the last file above a terminal would hand the whole window to
    /// the terminal, and the next file would have to make a pane to open in.
    /// The empty pane stays instead, where that file will go.
    pub(super) fn close_vacated(&mut self, pane: PaneId, held_documents: bool) {
        let scope: Option<Scope> = self.scope();
        let another = self.panes.panes().into_iter().any(|other| {
            other != pane
                && self
                    .panes
                    .pane(other)
                    .is_some_and(|other| other.serves(scope, Role::Editor))
        });
        if held_documents && !another {
            return;
        }
        self.panes.close_if_empty(pane);
    }
}
