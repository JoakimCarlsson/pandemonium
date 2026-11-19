//! Clearing the cookies and site storage of the browser: the question put to the reader first, and what answering it does.

use crate::areas::BrowserEvent;
use crate::browser::Command;
use crate::button::Variant;
use crate::git_prompts::{QuestionIds, question_of};
use crate::state::WorkspaceState;
use crate::target::Target;

/// What the question answers to.
const IDS: QuestionIds = QuestionIds {
    outside: Target::Browser(BrowserEvent::BrowserClearOutside),
    inside: Target::Browser(BrowserEvent::BrowserClearInside),
    confirm: Target::Browser(BrowserEvent::BrowserClearConfirm),
    cancel: Target::Browser(BrowserEvent::BrowserClearCancel),
};

impl WorkspaceState {
    /// Asks the reader whether to clear the cookies and site storage of the browser.
    pub fn browser_clear_asked(&mut self) {
        if let Some(browser) = self.browser.as_mut() {
            browser.clearing = true;
        }
    }

    /// Takes the question back without clearing anything, and says whether one was open.
    pub fn browser_clear_declined(&mut self) -> bool {
        self.browser
            .as_mut()
            .is_some_and(|browser| std::mem::take(&mut browser.clearing))
    }

    /// Clears the cookies, site storage and cache, as the reader agreed to.
    pub(crate) fn browser_clear_confirmed(&mut self) {
        if self.browser_clear_declined() {
            self.browser_queue(Command::ClearData);
        }
    }

    /// Whether the question about clearing is open.
    pub fn browser_clearing(&self) -> bool {
        self.browser
            .as_ref()
            .is_some_and(|browser| browser.clearing)
    }
}

/// Emits the question about clearing the cookies and site storage, while it is open; nothing otherwise.
#[track_caller]
pub fn clear_question(state: &WorkspaceState) {
    if state.browser_clearing() {
        question_of(
            IDS,
            "Clear cookies and site data?".to_owned(),
            "This signs you out of every site in the browser and removes what sites stored: cookies, local storage, IndexedDB and cached files. The page loads again afterwards.",
            "Clear",
            Variant::Destructive,
        );
    }
}

#[cfg(test)]
mod tests;
