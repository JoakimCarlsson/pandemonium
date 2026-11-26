//! Which profile of the browser's engine a tab's page lives in: the shared one, a private one kept in memory only, or, when the reader asks for it, one of the tab's project's own, so that sign-ins, cookies and storage do not reach from one project into another.

use crate::project::Project;
use crate::state::{Page, WorkspaceState};

/// The engine's name for the profile private tabs share; without `persist:` in front, the engine keeps it in memory only, and the helper empties it when the last page in it closes.
pub const PRIVATE_PARTITION: &str = "private";

/// The engine's name for the profile kept on disk for the project `project`, named by a number its address always gives, so it is the same after a restart.
pub fn project_partition(project: &Project) -> String {
    format!("persist:project-{:016x}", project.path_hash())
}

impl WorkspaceState {
    /// The engine's name for the profile the page of the tab numbered `tab` lives in, or nothing for the shared one: the private profile for a private tab, and while each project has a profile of its own, the profile of the project the tab's browser belongs to.
    pub fn browser_partition(&self, tab: u32) -> Option<String> {
        let held = self.browser_tab_anywhere(tab)?;
        if let Some(context) = &held.context {
            return Some(context.clone());
        }
        if held.private {
            return Some(PRIVATE_PARTITION.to_owned());
        }
        if !self.window.general.project_browser_profiles {
            return None;
        }
        self.browser_project(tab).map(project_partition)
    }

    /// Makes the tab numbered `opened` a page of the context whose profile is `partition`, which a script of an agent asked for.
    pub fn browser_tab_in_context(&mut self, opened: u32, partition: String) {
        if let Some(tab) = self.browser_tab(opened) {
            tab.context = Some(partition);
        }
    }

    /// Makes the tab numbered `opened`, which the page of the tab numbered `opener` asked for, private when the opener is and a page of the opener's context when it has one, so that a link or a window such a page opens stays in its profile and is not kept.
    pub fn browser_tab_inherit(&mut self, opener: u32, opened: u32) {
        let (private, context) = self
            .browser_tab_anywhere(opener)
            .map(|tab| (tab.private, tab.context.clone()))
            .unwrap_or_default();
        if let Some(tab) = self.browser_tab(opened) {
            tab.private |= private;
            if context.is_some() {
                tab.context = context;
            }
        }
    }

    /// The project the browser holding the tab numbered `tab` belongs to: the project of the session that owns it, or for the browser shown outside any session, the project the page shows.
    fn browser_project(&self, tab: u32) -> Option<&Project> {
        let shown = self
            .browser
            .as_ref()
            .is_some_and(|browser| browser.tab(tab).is_some());
        let scope = if shown {
            self.browser_scope
        } else {
            *self
                .browsers
                .iter()
                .find(|(_, browser)| browser.tab(tab).is_some())?
                .0
        };
        let owner = self.sessions.iter().find(|session| session.scope == scope);
        let project = match (owner, self.page) {
            (Some(session), _) => session.project?,
            (None, Page::Project(project) | Page::New(project)) if shown => project,
            _ => return None,
        };
        self.projects.get(project)
    }
}

#[cfg(test)]
mod tests;
