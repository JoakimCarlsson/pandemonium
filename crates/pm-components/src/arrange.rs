//! Arranging the projects: the menu on each one, putting them in another order by dragging, and taking one away.

use pm_input::FocusId;

use crate::state::{NEW_IN_BASE, Outgoing, PROJECT_BASE, Page, SESSION_BASE, WorkspaceState};

/// The first of the buttons opening the menu of a project; the rest follow it.
pub const PROJECT_MORE_BASE: u64 = 1200;

/// The row of a project's menu that starts a session in it.
pub const PROJECT_MENU_NEW: FocusId = FocusId(305);

/// The row of a project's menu that takes it out of the window.
pub const PROJECT_MENU_REMOVE: FocusId = FocusId(306);

/// The menu open over a project, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectMenu {
    /// The project, by position.
    pub project: usize,
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
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

/// Whether `id` names a row of a project's menu.
pub fn is_project_menu_item(id: FocusId) -> bool {
    id == PROJECT_MENU_NEW || id == PROJECT_MENU_REMOVE
}

impl WorkspaceState {
    /// Which project's menu button `id` is, if it is one.
    pub fn more_button_of(&self, id: FocusId) -> Option<usize> {
        id.0.checked_sub(PROJECT_MORE_BASE)
            .map(|index| index as usize)
            .filter(|index| *index < self.projects.len())
    }

    /// Which project's header `id` is, if it is one.
    pub fn header_of(&self, id: FocusId) -> Option<usize> {
        id.0.checked_sub(PROJECT_BASE)
            .map(|index| index as usize)
            .filter(|index| *index < self.projects.len())
    }

    /// The project the element `id` belongs to: its header, its row for new work or one of its sessions.
    pub fn project_at(&self, id: FocusId) -> Option<usize> {
        let within = |base: u64, count: usize| {
            id.0.checked_sub(base)
                .map(|index| index as usize)
                .filter(|index| *index < count)
        };
        within(PROJECT_BASE, self.projects.len())
            .or_else(|| within(NEW_IN_BASE, self.projects.len()))
            .or_else(|| {
                within(SESSION_BASE, self.sessions.len())
                    .map(|position| self.sessions[position].project)
            })
    }

    /// The state with the menu of the project at `project` open at `at`.
    pub fn with_project_menu(mut self, project: usize, at: (f32, f32)) -> Self {
        if project < self.projects.len() {
            self.menu = None;
            self.project_menu = Some(ProjectMenu { project, at });
        }
        self
    }

    /// The state after the row `row` of the open project menu was pressed.
    pub(crate) fn project_menu_pressed(mut self, row: FocusId) -> Self {
        let Some(open) = self.project_menu.take() else {
            return self;
        };
        if row == PROJECT_MENU_NEW {
            self.folded[open.project] = false;
            self.page = Page::New(open.project);
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
        self.folded = order.iter().map(|old| self.folded[*old]).collect();
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
        self.folded.remove(index);
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
            match map.get(session.project).copied().flatten() {
                Some(project) => {
                    session.project = project;
                    moved.push(Some(kept.len()));
                    kept.push(session);
                }
                None => moved.push(None),
            }
        }
        self.sessions = kept;
        self.page = match self.page {
            Page::Session(position) => match moved.get(position).copied().flatten() {
                Some(position) => Page::Session(position),
                None => Page::New(0),
            },
            Page::New(project) => Page::New(map.get(project).copied().flatten().unwrap_or(0)),
            Page::History => Page::History,
        };
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
        self.skills_of = self
            .skills_of
            .and_then(|(agent, project)| Some((agent, map.get(project).copied().flatten()?)));
        if moved.iter().enumerate().any(|(old, new)| *new != Some(old)) {
            self.outbox.push(Outgoing::Reindexed(moved));
        }
        self.project_drag = None;
        self.sync_context();
    }
}
