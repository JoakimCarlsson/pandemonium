//! What the page for starting work hands to the session started from it: the browser it held and the shells of its dock.
//!
//! On the page for starting work, the browser is the one held under `NO_SCOPE` and the dock shows the shells of the project's `home` session. Starting a session from that page moves both into the new session, so the pages and the running shells follow the reader instead of staying behind.

use crate::browser_scope::NO_SCOPE;
use crate::state::{Page, WorkspaceState};

impl WorkspaceState {
    /// Moves what the page for starting work in the project at `project` holds into the session at `session`: the browser under `NO_SCOPE`, with its tabs, the tab shown and its developer tools, and the shells of the project's `home` session, with their numbers, output and the shell shown.
    ///
    /// The page for starting work and the `home` session are left with neither. Nothing happens when `session` is not a session.
    pub(crate) fn hand_over_new_page(&mut self, project: usize, session: usize) {
        let Some(scope) = self.sessions.get(session).map(|held| held.scope) else {
            return;
        };
        self.hand_over_browser(scope);
        if let Some(home) = self.review_of(Page::New(project)) {
            self.hand_over_shells(home, session);
        }
    }

    /// Makes the browser held under `NO_SCOPE` the browser of `scope`, whether it is the one shown or one set aside.
    fn hand_over_browser(&mut self, scope: u64) {
        if self.browser_scope == NO_SCOPE {
            if self.browser.is_some() {
                self.browser_scope = scope;
            }
        } else if let Some(browser) = self.browsers.remove(&NO_SCOPE) {
            self.browsers.insert(scope, browser);
        }
    }

    /// Moves the shells of the session at `from` into the session at `to`, keeping the shell shown, unless `to` holds shells of its own.
    fn hand_over_shells(&mut self, from: usize, to: usize) {
        if from == to
            || self
                .sessions
                .get(to)
                .is_none_or(|held| !held.terminals.is_empty())
        {
            return;
        }
        let Some(source) = self.sessions.get_mut(from) else {
            return;
        };
        let terminals = std::mem::take(&mut source.terminals);
        let active = std::mem::take(&mut source.terminal_active);
        let target = &mut self.sessions[to];
        target.terminals = terminals;
        target.terminal_active = active;
    }
}

#[cfg(test)]
mod tests;
