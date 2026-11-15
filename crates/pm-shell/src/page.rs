//! Which page the window shows and the trail of pages the reader has been
//! to, walked back and forward like the history of a browser.

/// A page of the workspace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Page {
    /// The session view with no session open.
    #[default]
    Home,
    /// The session view showing the session with this number.
    Session(u64),
    /// Every session of every project, newest first.
    History,
    /// The extensions installed on the server and those it can install.
    Extensions,
    /// The overview of the project at this address.
    Overview(String),
}

impl Page {
    /// Whether the page is the session view, with or without a session.
    pub fn is_session(&self) -> bool {
        matches!(self, Self::Home | Self::Session(_))
    }
}

/// The pages visited in order, and which of them is showing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trail {
    /// The pages, oldest first.
    places: Vec<Page>,
    /// The position in `places` of the page showing.
    at: usize,
}

impl Trail {
    /// Records `page` as the newest, dropping the pages that were ahead of
    /// the one showing; a page equal to the one showing is not recorded twice.
    pub fn visit(&mut self, page: Page) {
        if self.places.get(self.at) == Some(&page) {
            return;
        }
        if !self.places.is_empty() {
            self.places.truncate(self.at + 1);
        }
        self.places.push(page);
        self.at = self.places.len() - 1;
    }

    /// Steps back to the page before the one showing and returns it, or
    /// nothing at the start of the trail.
    pub fn back(&mut self) -> Option<Page> {
        let before = self.at.checked_sub(1)?;
        self.at = before;
        self.places.get(before).cloned()
    }

    /// Steps forward to the page after the one showing and returns it, or
    /// nothing at the end of the trail.
    pub fn forward(&mut self) -> Option<Page> {
        let page = self.places.get(self.at + 1).cloned()?;
        self.at += 1;
        Some(page)
    }

    /// Whether there is a page to go back to.
    pub fn can_go_back(&self) -> bool {
        self.at > 0
    }

    /// Whether there is a page to go forward to.
    pub fn can_go_forward(&self) -> bool {
        self.at + 1 < self.places.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Going back and forward walks the pages visited, and visiting from the
    /// middle drops the pages ahead.
    #[test]
    fn walks_back_and_forward() {
        let mut trail = Trail::default();
        trail.visit(Page::Session(1));
        trail.visit(Page::History);
        trail.visit(Page::History);
        trail.visit(Page::Extensions);
        assert_eq!(trail.back(), Some(Page::History));
        assert_eq!(trail.back(), Some(Page::Session(1)));
        assert_eq!(trail.back(), None);
        assert!(trail.can_go_forward());
        assert_eq!(trail.forward(), Some(Page::History));
        trail.visit(Page::Overview("~/repos/void".to_owned()));
        assert!(!trail.can_go_forward());
        assert_eq!(trail.back(), Some(Page::History));
    }
}
