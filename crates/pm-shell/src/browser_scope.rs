//! Whose browser the window shows: every session keeps tabs of its own, and showing another session swaps the tabs shown.
//!
//! The browser of the session shown is [`crate::Workspace::browser`], which the pages read. The browsers of the other sessions wait on a [`BrowserShelf`], by the number of the session that owns each, until their session is shown again; the browser of no session, shown on the start page and the other pages, waits under [`NO_SCOPE`].

use std::collections::HashMap;

use crate::browser::{Browser, BrowserTab, SavedTab};

/// The scope of the browser that belongs to no session.
pub const NO_SCOPE: u64 = 0;

/// A browser as it is kept across a restart, with the session it belongs to.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SavedBrowser {
    /// The number of the session it belongs to, or [`NO_SCOPE`].
    pub session: u64,
    /// Its tabs.
    pub tabs: Vec<SavedTab>,
    /// The index of the tab shown.
    pub active: usize,
    /// Whether the developer tools were open.
    pub tools: bool,
    /// How wide the developer tools were.
    pub tools_width: f32,
}

/// The browsers of the sessions not shown, and whose browser is shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BrowserShelf {
    /// The scope of the browser shown.
    pub scope: u64,
    /// The browsers set aside, by the scope of their session.
    pub kept: HashMap<u64, Browser>,
}

impl BrowserShelf {
    /// Makes `shown` the browser of `scope`, putting the browser it held on the shelf for its own scope.
    pub fn swap(&mut self, shown: &mut Option<Browser>, scope: u64) {
        if self.scope == scope {
            return;
        }
        if let Some(browser) = shown.take() {
            self.kept.insert(self.scope, browser);
        }
        *shown = self.kept.remove(&scope);
        self.scope = scope;
    }

    /// Forgets the browsers of sessions not among `sessions`, keeping the one of no session.
    pub fn retain(&mut self, sessions: &[u64]) {
        self.kept
            .retain(|scope, _| *scope == NO_SCOPE || sessions.contains(scope));
    }

    /// Runs `act` on the browser that holds the tab numbered `tab`, whether it is `shown` or set aside; nothing when no browser holds it.
    pub fn with_tab<R>(
        &mut self,
        shown: &mut Option<Browser>,
        tab: u32,
        act: impl FnOnce(&mut Browser) -> R,
    ) -> Option<R> {
        if let Some(browser) = shown.as_mut().filter(|browser| browser.tab(tab).is_some()) {
            return Some(act(browser));
        }
        self.kept
            .values_mut()
            .find(|browser| browser.tab(tab).is_some())
            .map(act)
    }

    /// The tab numbered `tab` of any browser, `shown` or set aside.
    pub fn tab<'a>(&'a self, shown: &'a Option<Browser>, tab: u32) -> Option<&'a BrowserTab> {
        shown
            .iter()
            .chain(self.kept.values())
            .find_map(|browser| browser.tab(tab))
    }

    /// The numbers of every tab of every browser, for telling which pages are still wanted.
    pub fn tab_numbers(&self, shown: &Option<Browser>) -> Vec<u32> {
        shown
            .iter()
            .chain(self.kept.values())
            .flat_map(|browser| browser.tabs.iter().map(|tab| tab.id))
            .collect()
    }

    /// Hands the browser of no session to the session `to`, as starting a session from the start page does; nothing when `to` already has one.
    pub fn hand_over(&mut self, shown: &mut Option<Browser>, to: u64) {
        let target_has = self.kept.contains_key(&to) || (self.scope == to && shown.is_some());
        if target_has || to == NO_SCOPE {
            return;
        }
        if self.scope == NO_SCOPE {
            if shown.is_some() {
                self.scope = to;
            }
        } else if let Some(browser) = self.kept.remove(&NO_SCOPE) {
            self.kept.insert(to, browser);
        }
    }

    /// Every browser as it is kept across a restart.
    pub fn saved(&self, shown: &Option<Browser>) -> Vec<SavedBrowser> {
        let save = |scope: u64, browser: &Browser| SavedBrowser {
            session: scope,
            tabs: browser.saved_tabs(),
            active: browser.active,
            tools: browser.tools,
            tools_width: browser.tools_width,
        };
        let mut saved: Vec<SavedBrowser> = shown
            .iter()
            .map(|browser| save(self.scope, browser))
            .chain(
                self.kept
                    .iter()
                    .map(|(scope, browser)| save(*scope, browser)),
            )
            .collect();
        saved.sort_by_key(|browser| browser.session);
        saved
    }

    /// Puts back the browsers `saved` on the shelf, each until its session is shown.
    pub fn restore(&mut self, shown: &mut Option<Browser>, saved: &[SavedBrowser]) {
        for kept in saved {
            let Some(browser) =
                Browser::restored(&kept.tabs, kept.active, kept.tools, kept.tools_width)
            else {
                continue;
            };
            if kept.session == self.scope {
                *shown = Some(browser);
            } else {
                self.kept.insert(kept.session, browser);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Showing another session puts the browser away, and showing it again brings it back.
    #[test]
    fn each_session_keeps_its_browser() {
        let mut shelf = BrowserShelf::default();
        let mut shown = None;
        shelf.swap(&mut shown, 7);
        shown = Some(Browser::new("http://localhost:1", None));
        shelf.swap(&mut shown, 8);
        assert!(shown.is_none());
        shelf.swap(&mut shown, 7);
        assert_eq!(
            shown.as_ref().map(|b| b.shown().address.clone()),
            Some("http://localhost:1".to_owned())
        );
    }

    /// The browser of the start page goes to the session started from it.
    #[test]
    fn the_start_page_hands_its_browser_over() {
        let mut shelf = BrowserShelf::default();
        let mut shown = Some(Browser::new("http://localhost:5173", None));
        shelf.hand_over(&mut shown, 9);
        assert_eq!(shelf.scope, 9);
        shelf.swap(&mut shown, NO_SCOPE);
        assert!(shown.is_none());
        shelf.swap(&mut shown, 9);
        assert!(shown.is_some());
    }

    /// A tab of a browser set aside is still found, and gone sessions are forgotten.
    #[test]
    fn tabs_are_found_anywhere() {
        let mut shelf = BrowserShelf::default();
        let mut shown = Some(Browser::new("about:blank", None));
        let id = shown.as_ref().map(Browser::active_id).unwrap_or_default();
        shelf.swap(&mut shown, 3);
        assert!(shelf.tab(&shown, id).is_some());
        assert_eq!(shelf.with_tab(&mut shown, id, |b| b.tabs.len()), Some(1));
        shelf.retain(&[3]);
        assert!(shelf.tab(&shown, id).is_some());
        shelf.scope = 3;
        shelf.kept.insert(5, Browser::new("about:blank", None));
        shelf.retain(&[]);
        assert!(shelf.kept.contains_key(&NO_SCOPE));
        assert!(!shelf.kept.contains_key(&5));
    }

    /// Saved browsers come back on the shelf under their sessions.
    #[test]
    fn browsers_are_saved_and_restored() {
        let mut shelf = BrowserShelf::default();
        let mut shown = Some(Browser::new("http://localhost:2", None));
        shelf.swap(&mut shown, 4);
        let saved = shelf.saved(&shown);
        assert_eq!(saved.len(), 1);
        let mut again = BrowserShelf::default();
        let mut none = None;
        again.restore(&mut none, &saved);
        assert!(none.is_some());
    }
}
