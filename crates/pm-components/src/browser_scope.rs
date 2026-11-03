//! Whose browser the window shows: every session keeps tabs of its own, and switching session swaps the tabs shown.
//!
//! The browser of the session being shown is `WorkspaceState::browser`, which the rest of the window reads and changes. The browsers of the other sessions wait in `WorkspaceState::browsers`, by the scope of the session that owns each, until their session is shown again.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::browser::Browser;
use crate::browser_tabs::SavedTab;
use crate::state::{Page, Tab, WorkspaceState};

/// The scope of the browser that belongs to no session, held while no session is shown.
pub const NO_SCOPE: u64 = 0;

/// The scope the next session gets.
static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);

/// The number the next tab of any browser gets.
static NEXT_TAB: AtomicU32 = AtomicU32::new(1);

/// A scope no other session has had in this window, which names a session's browser for as long as the window runs, however the sessions are renumbered.
pub fn next_scope() -> u64 {
    NEXT_SCOPE.fetch_add(1, Ordering::Relaxed)
}

/// A tab number no other tab of the window has had, so that the pages of every session's tabs can be told apart by number.
pub fn next_tab_number() -> u32 {
    NEXT_TAB.fetch_add(1, Ordering::Relaxed)
}

/// What `WorkspaceState::browser_scope_enter` took the window out of, to be handed back to `browser_scope_leave`.
pub struct Entered {
    /// The scope whose browser was in `WorkspaceState::browser` before.
    shown: u64,
    /// The tab of the page that was shown before, which entering a scope other than the shown one must not change.
    tab: Tab,
}

/// How a browser was left, as saved: its tabs, the index of the one shown, whether its developer tools were open and how wide they were.
pub type SavedBrowserParts = (Vec<SavedTab>, usize, bool, f32);

impl WorkspaceState {
    /// The scope of the session being shown, or `NO_SCOPE` on any other page.
    fn wanted_browser_scope(&self) -> u64 {
        match self.page {
            Page::Session(position) => self
                .sessions
                .get(position)
                .map_or(NO_SCOPE, |session| session.scope),
            _ => NO_SCOPE,
        }
    }

    /// Makes `WorkspaceState::browser` the browser of `scope`, keeping the one it held for its own session.
    fn browser_swap(&mut self, scope: u64) {
        if self.browser_scope == scope {
            return;
        }
        if let Some(browser) = self.browser.take() {
            self.browsers.insert(self.browser_scope, browser);
        }
        self.browser = self.browsers.remove(&scope);
        self.browser_scope = scope;
    }

    /// Whether the browser in `browser` is not the one of the page shown, or a browser of a session that is gone waits.
    pub fn browser_rescope_needed(&self) -> bool {
        self.browser_scope != self.wanted_browser_scope()
            || self.browsers.keys().any(|scope| {
                *scope != NO_SCOPE && !self.sessions.iter().any(|session| session.scope == *scope)
            })
    }

    /// Puts the browser of the session being shown where the window reads it, and forgets the browsers of sessions that are gone. Called whenever the page may have changed.
    pub fn rescope_browser(&mut self) {
        let wanted = self.wanted_browser_scope();
        self.browser_swap(wanted);
        if self.browser.is_none() && self.tab == Tab::Browser {
            self.tab = Tab::Chat;
        }
        let kept: Vec<u64> = self.sessions.iter().map(|session| session.scope).collect();
        self.browsers
            .retain(|scope, _| *scope == NO_SCOPE || kept.contains(scope));
    }

    /// Takes the browser the window was restored with, which belongs to the session shown when the window closed, as the browser of the session at `position`.
    pub fn browser_adopted_by(&mut self, position: usize) {
        if let Some(session) = self.sessions.get(position) {
            self.browser_scope = session.scope;
        }
    }

    /// Makes the browser of the session at `position` the one every browser method works on, until `browser_scope_leave`; for work on the tabs of a session the reader is not looking at, such as an agent's tool call.
    ///
    /// The page the reader is on is left as it is.
    pub fn browser_scope_enter(&mut self, position: usize) -> Option<Entered> {
        let scope = self.sessions.get(position)?.scope;
        let entered = Entered {
            shown: self.browser_scope,
            tab: self.tab,
        };
        self.browser_swap(scope);
        Some(entered)
    }

    /// Puts the browser that `browser_scope_enter` set aside back, and the page the reader was on, unless the work was on the browser being shown.
    pub fn browser_scope_leave(&mut self, entered: Entered) {
        let worked_on = self.browser_scope;
        self.browser_swap(entered.shown);
        if worked_on != entered.shown {
            self.tab = entered.tab;
        }
    }

    /// The position of the session that holds the shell `tab`.
    pub fn session_of_terminal(&self, tab: u64) -> Option<usize> {
        self.sessions
            .iter()
            .position(|session| session.terminals.iter().any(|held| held.id == tab))
    }

    /// The tab numbered `tab` of any session's browser, the one shown or one set aside.
    pub fn browser_tab_anywhere(&self, tab: u32) -> Option<&crate::browser_tabs::BrowserTab> {
        self.browser
            .iter()
            .chain(self.browsers.values())
            .find_map(|browser| browser.tab(tab))
    }

    /// Runs `act` with the browser that holds the tab numbered `tab` as the one every browser method works on, then puts back the browser and the page the reader was on; nothing when no browser holds the tab.
    ///
    /// For what a page reports, which may come from a session the reader is not looking at.
    pub fn browser_scoped_to_tab<R>(
        &mut self,
        tab: u32,
        act: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        let shown = self.browser_scope;
        let page_tab = self.tab;
        let owner = if self.browser.as_ref().is_some_and(|b| b.tab(tab).is_some()) {
            shown
        } else {
            *self
                .browsers
                .iter()
                .find(|(_, browser)| browser.tab(tab).is_some())?
                .0
        };
        self.browser_swap(owner);
        let out = act(self);
        self.browser_swap(shown);
        if owner != shown {
            self.tab = page_tab;
        }
        Some(out)
    }

    /// The numbers of every tab of every session, for telling which pages are still wanted.
    pub fn browser_tab_numbers(&self) -> Vec<u32> {
        self.browser
            .iter()
            .chain(self.browsers.values())
            .flat_map(|browser| browser.tabs.iter().map(|tab| tab.id))
            .collect()
    }

    /// The browser of the session at `position` as it would be saved, whether or not it is the one being shown.
    pub fn session_browser_saved(&self, position: usize) -> Option<SavedBrowserParts> {
        let scope = self.sessions.get(position)?.scope;
        let browser = if scope == self.browser_scope {
            self.browser.as_ref()
        } else {
            self.browsers.get(&scope)
        }?;
        Some(saved_parts(browser))
    }

    /// Gives the session at `position` the browser it was left with, unless it already has one.
    pub fn restore_session_browser(
        &mut self,
        position: usize,
        tabs: &[SavedTab],
        active: usize,
        tools: bool,
        tools_width: f32,
    ) {
        let Some(scope) = self.sessions.get(position).map(|session| session.scope) else {
            return;
        };
        let held = scope == self.browser_scope && self.browser.is_some();
        if held || self.browsers.contains_key(&scope) {
            return;
        }
        let Some(browser) = Browser::restored(tabs, active, tools, tools_width) else {
            return;
        };
        if scope == self.browser_scope {
            self.browser = Some(browser);
        } else {
            self.browsers.insert(scope, browser);
        }
    }
}

/// The parts of `browser` that are saved.
pub(crate) fn saved_parts(browser: &Browser) -> SavedBrowserParts {
    let tabs = browser
        .tabs
        .iter()
        .map(|tab| SavedTab {
            address: tab.address.text().to_owned(),
            host: tab.host.clone(),
            agent: tab.agent,
            locked: tab.locked,
        })
        .collect();
    (tabs, browser.active, browser.tools, browser.tools_width)
}

/// The browsers set aside, by the scope of their session, for state that has none.
pub(crate) fn no_browsers() -> HashMap<u64, Browser> {
    HashMap::new()
}

#[cfg(test)]
mod tests;
