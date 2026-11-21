//! Arranging the projects: the menu on each one, putting them in another order by dragging, and taking one away.

use crate::areas::{MenuEvent, SidebarEvent};
use crate::git_prompts::Prompt;
use crate::state::{Outgoing, Page, WorkspaceState};
use crate::target::Target;

/// The menu open over a session, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SessionMenu {
    /// The session, by position.
    pub session: usize,
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
}

/// The menu open over a project, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectMenu {
    /// The project, by position.
    pub project: usize,
    /// Where it opened, in logical pixels, or nothing when it hangs from the project's button.
    pub at: Option<(f32, f32)>,
}

/// A project being dragged to another place in the list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectDrag {
    /// The project being dragged, by position.
    pub from: usize,
    /// The project whose place it takes when it is let go.
    pub over: usize,
    /// Where the pointer is, in logical pixels, which is where the dragged header is drawn.
    pub at: (f32, f32),
}

/// Whether `id` names the row of a session's menu.
pub fn is_session_menu_item(id: Target) -> bool {
    [
        Target::Menu(MenuEvent::SessionMenuRead),
        Target::Menu(MenuEvent::SessionMenuClose),
        Target::Menu(MenuEvent::SessionMenuRemove),
        Target::Menu(MenuEvent::SessionMenuCopyPath),
        Target::Menu(MenuEvent::SessionMenuCopyBranch),
        Target::Menu(MenuEvent::SessionMenuRename),
        Target::Menu(MenuEvent::SessionMenuOpenFolder),
    ]
    .contains(&id)
}

/// Whether `id` names a row of a project's menu.
pub fn is_project_menu_item(id: Target) -> bool {
    id == Target::Menu(MenuEvent::ProjectMenuNew)
        || id == Target::Menu(MenuEvent::ProjectMenuRemove)
}

impl WorkspaceState {
    /// Which project's header `id` is, if it is one.
    pub fn header_of(&self, id: Target) -> Option<usize> {
        match id {
            Target::Sidebar(SidebarEvent::Project(index)) if index < self.projects.len() => {
                Some(index)
            }
            _ => None,
        }
    }

    /// The project the element `id` belongs to: its header, its row for new work or one of its sessions.
    pub fn project_at(&self, id: Target) -> Option<usize> {
        match id {
            Target::Sidebar(SidebarEvent::Project(index))
            | Target::Sidebar(SidebarEvent::NewIn(index))
                if index < self.projects.len() =>
            {
                Some(index)
            }
            Target::Sidebar(SidebarEvent::Session(position)) if position < self.sessions.len() => {
                self.sessions[position].project
            }
            _ => None,
        }
    }

    /// The state with the menu of the session at `session` open at `at`.
    pub fn with_session_menu(mut self, session: usize, at: (f32, f32)) -> Self {
        if session < self.sessions.len() {
            self.menu = None;
            self.project_menu = None;
            self.message_menu = None;
            self.session_menu = Some(SessionMenu { session, at });
        }
        self
    }

    /// Whether the session at `position` works on a machine other than this one.
    pub fn is_remote_session(&self, position: usize) -> bool {
        self.sessions
            .get(position)
            .and_then(|held| self.machine_of(held))
            .is_some_and(|machine| machine.is_some())
    }

    /// The state after the row `row` of the open session menu was pressed.
    pub(crate) fn session_menu_pressed(mut self, row: Target) -> Self {
        let Some(open) = self.session_menu.take() else {
            return self;
        };
        let Some(held) = self.sessions.get(open.session) else {
            return self;
        };
        match row {
            Target::Menu(MenuEvent::SessionMenuRead) => {
                let held = &mut self.sessions[open.session];
                if held.unread() {
                    held.mark_read();
                } else {
                    held.marked_unread = true;
                    held.hold_unread = self.page == Page::Session(open.session);
                }
                self
            }
            Target::Menu(MenuEvent::SessionMenuCopyPath | MenuEvent::SessionMenuCopyBranch) => {
                let text = if row == Target::Menu(MenuEvent::SessionMenuCopyPath) {
                    held.directory.clone()
                } else {
                    held.branch.clone()
                };
                self.outbox.push(Outgoing::Copy { text });
                self
            }
            Target::Menu(MenuEvent::SessionMenuRename) => {
                self.git.rename = rustle::TextFieldState::new(held.title.clone());
                self.ask(Prompt::Rename(open.session));
                self
            }
            Target::Menu(MenuEvent::SessionMenuOpenFolder) => {
                self.outbox.push(Outgoing::OpenFolder {
                    session: open.session,
                });
                self
            }
            Target::Menu(MenuEvent::SessionMenuRemove) => {
                if held.is_chat() {
                    self.ask(Prompt::Close(open.session));
                }
                self
            }
            _ if held.base.is_some() && !held.changes.is_empty() => {
                self.ask(Prompt::Close(open.session));
                self
            }
            _ => self.closed_session(open.session, true, false),
        }
    }

    /// The state with the session at `position` closed, or, when `ask` is set, the server told to close it, removing the worktree even with changes that are not committed when `discard` is set, and the session left open until the server has.
    pub fn closed_session(mut self, position: usize, ask: bool, discard: bool) -> Self {
        self.close_session(position, ask, discard);
        self
    }

    /// Closes the session at `position`, or, when `ask` is set, tells the server to close it, removing the worktree even with changes that are not committed when `discard` is set, and leaves it open until the server has.
    pub(crate) fn close_session(&mut self, position: usize, ask: bool, discard: bool) {
        let Some(held) = self.sessions.get_mut(position) else {
            return;
        };
        if ask {
            self.outbox.push(Outgoing::Close {
                session: position,
                discard,
            });
            return;
        }
        held.closed = true;
        let (project, machine) = (held.project, held.machine.clone());
        if self.page == Page::Session(position) {
            match project {
                Some(project) => self.start_in(project, None),
                None => self.start_chat_on(machine),
            }
            self.sync_context();
        }
    }

    /// The state with the reader asked whether to close the session at `position` though its worktree has changes that are not committed, or whether to remove the chat there with its scratch folder.
    pub fn asking_to_close(mut self, position: usize) -> Self {
        if position < self.sessions.len() {
            self.ask(Prompt::Close(position));
        }
        self
    }

    /// The state with the menu of the project at `project` open at `at`, or hanging from the
    /// project's button without a point.
    pub fn with_project_menu(mut self, project: usize, at: Option<(f32, f32)>) -> Self {
        if project < self.projects.len() {
            self.menu = None;
            self.session_menu = None;
            self.message_menu = None;
            self.project_menu = Some(ProjectMenu { project, at });
        }
        self
    }

    /// The state after the row `row` of the open project menu was pressed.
    pub(crate) fn project_menu_pressed(mut self, row: Target) -> Self {
        let Some(open) = self.project_menu.take() else {
            return self;
        };
        if row == Target::Menu(MenuEvent::ProjectMenuNew) {
            self.start_in(open.project, None);
            self.sync_context();
            self
        } else {
            self.without_project(open.project)
        }
    }

    /// The state with the project at `from` being dragged to the place of the one at `over`, with the pointer at `at`.
    pub fn dragged_over(mut self, from: usize, over: usize, at: (f32, f32)) -> Self {
        if from < self.projects.len() && over < self.projects.len() {
            self.project_drag = Some(ProjectDrag { from, over, at });
        }
        self
    }

    /// The projects in the order the sidebar lists them: the order they are in, with the one being dragged already in the place it would take.
    pub fn display_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.projects.len()).collect();
        if let Some(drag) = self.project_drag
            && drag.from < order.len()
            && drag.over < order.len()
        {
            let moved = order.remove(drag.from);
            order.insert(drag.over, moved);
        }
        order
    }

    /// The state after a dragged project was let go: it takes the place of the one it is over.
    pub fn dropped(mut self) -> Self {
        match self.project_drag.take() {
            Some(drag) if drag.from != drag.over => self.moved_project(drag.from, drag.over),
            _ => self,
        }
    }

    /// The state with the project at `from` moved to the place `to`, the ones between shifting by one.
    pub fn moved_project(mut self, from: usize, to: usize) -> Self {
        let count = self.projects.len();
        if from >= count || to >= count || from == to {
            return self;
        }
        let mut order: Vec<usize> = (0..count).collect();
        let moved = order.remove(from);
        order.insert(to, moved);
        let mut map = vec![None; count];
        for (new, old) in order.iter().enumerate() {
            map[*old] = Some(new);
        }
        self.projects = order
            .iter()
            .map(|old| self.projects[*old].clone())
            .collect();
        self.remap_projects(&map);
        self
    }

    /// The state without the project at `index`, and without its sessions, which the server keeps.
    pub fn without_project(mut self, index: usize) -> Self {
        let count = self.projects.len();
        if index >= count {
            return self;
        }
        self.projects.remove(index);
        let map: Vec<Option<usize>> = (0..count)
            .map(|old| match old.cmp(&index) {
                std::cmp::Ordering::Less => Some(old),
                std::cmp::Ordering::Equal => None,
                std::cmp::Ordering::Greater => Some(old - 1),
            })
            .collect();
        self.remap_projects(&map);
        self
    }

    /// Brings everything that holds a project's position up to date after the projects changed places or one went: `map` says, for each old position, the new one, or nothing for a project that is gone.
    fn remap_projects(&mut self, map: &[Option<usize>]) {
        let mut moved: Vec<Option<usize>> = Vec::with_capacity(self.sessions.len());
        let mut kept = Vec::new();
        for mut session in std::mem::take(&mut self.sessions) {
            let placed = match session.project {
                Some(project) => map.get(project).copied().flatten().map(Some),
                None => Some(None),
            };
            match placed {
                Some(project) => {
                    session.project = project;
                    moved.push(Some(kept.len()));
                    kept.push(session);
                }
                None => {
                    moved.push(None);
                    self.outbox.extend(
                        session
                            .terminals
                            .iter()
                            .map(|tab| Outgoing::CloseTerminal { tab: tab.id }),
                    );
                }
            }
        }
        self.sessions = kept;
        self.remap_debug(&moved);
        let shown = match self.page {
            Page::Session(position) => moved.get(position).copied().flatten().map(Page::Session),
            Page::New(project) => map.get(project).copied().flatten().map(Page::New),
            Page::Project(project) => map.get(project).copied().flatten().map(Page::Project),
            page @ (Page::Chat | Page::History | Page::Extensions) => Some(page),
        };
        match shown {
            Some(page) => self.page = page,
            None => self.start_anew(),
        }
        self.opened = self
            .opened
            .iter()
            .filter_map(|(session, reply, step)| {
                Some((moved.get(*session).copied().flatten()?, *reply, *step))
            })
            .collect();
        self.branches_of = self
            .branches_of
            .and_then(|project| map.get(project).copied().flatten());
        self.continuing = crate::continue_chat::Choice::default();
        if moved.iter().enumerate().any(|(old, new)| *new != Some(old)) {
            self.outbox.push(Outgoing::Reindexed(moved));
        }
        self.project_drag = None;
        self.sync_context();
    }
}

/// `menu` holding the rows of the menu of a project: start a session in it, or take it out of
/// the window.
pub fn project_menu(menu: crate::Menu) -> crate::Menu {
    menu.item(
        "New session",
        None,
        Target::Menu(MenuEvent::ProjectMenuNew),
        false,
    )
    .separator()
    .item(
        "Remove workspace",
        None,
        Target::Menu(MenuEvent::ProjectMenuRemove),
        false,
    )
}
