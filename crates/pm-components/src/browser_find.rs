//! Finding text in the page of the browser: the search the engine runs, and the bar over the top right corner of the page that drives it.

use pm_field::{Edit, Field};
use rustle::{BoxAlignment, Modifier};

use crate::kit::{Flex, IconName, Interactive, Radius, Role, Styled, block, row, sp};

use crate::browser::{
    BROWSER_FIND_BAR, BROWSER_FIND_CLOSE, BROWSER_FIND_FIELD, BROWSER_FIND_NEXT,
    BROWSER_FIND_PREVIOUS, Command,
};
use crate::find::SHADOW;
use crate::parts::line;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::{Button, Input};

/// How wide the line the text is typed in is when there is room, in logical pixels.
const FIELD_WIDTH: f32 = 220.0;

/// How narrow the line the text is typed in is made when the page is narrow, in logical pixels.
const FIELD_LEAST: f32 = 60.0;

/// How wide the bar is apart from the line the text is typed in, in logical pixels.
const BAR_EXTRA: f32 = 194.0;

/// How far the bar is from the right edge of the page, in logical pixels.
const EDGE: f32 = 24.0;

/// How wide the count of matches is, in logical pixels.
const COUNT_WIDTH: f32 = 84.0;

/// The search of the page: what is looked for and what the engine found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageSearch {
    /// What is looked for.
    pub query: Field,
    /// Which match is the current one, counting from 1, and 0 for none.
    pub active: u32,
    /// How many matches the engine found.
    pub total: u32,
    /// Whether the engine has counted the matches of the query as it stands.
    pub counted: bool,
}

impl PageSearch {
    /// What the bar says of the matches, and in which tone: nothing before they are counted, that there are none, or which is current of how many.
    pub fn tally(&self) -> (String, Role) {
        if self.query.text().is_empty() || !self.counted {
            (String::new(), Role::Subtle)
        } else if self.total == 0 {
            ("No results".to_owned(), Role::Danger)
        } else {
            (format!("{} of {}", self.active, self.total), Role::Muted)
        }
    }
}

impl WorkspaceState {
    /// Whether the bar for finding in the page shows.
    pub fn browser_find_open(&self) -> bool {
        self.browser
            .as_ref()
            .is_some_and(|browser| browser.search.is_some())
    }

    /// Shows the bar for finding in the page with the text looked for last marked, and looks for it again.
    pub fn browser_find_opened(&mut self) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        let tab = &mut **browser;
        let search = tab.search.get_or_insert_with(PageSearch::default);
        search.query.apply(Edit::SelectAll);
        if !search.query.text().is_empty() {
            search.counted = false;
            tab.commands.push(Command::Find {
                text: search.query.text().to_owned(),
                forward: true,
                again: false,
            });
        }
    }

    /// Hides the bar and clears the marks in the page.
    pub fn browser_find_closed(&mut self) {
        if let Some(browser) = self.browser.as_mut()
            && browser.search.take().is_some()
        {
            browser.commands.push(Command::StopFind);
        }
    }

    /// Goes to the next match, or to the previous one when `back`; shows the bar when it is not shown.
    pub fn browser_find_stepped(&mut self, back: bool) {
        if !self.browser_find_open() {
            self.browser_find_opened();
            return;
        }
        if let Some(browser) = self.browser.as_mut()
            && let tab = &mut **browser
            && let Some(search) = tab.search.as_ref()
            && !search.query.text().is_empty()
        {
            tab.commands.push(Command::Find {
                text: search.query.text().to_owned(),
                forward: !back,
                again: true,
            });
        }
    }

    /// Applies `edit` to the text looked for, and looks for the new text when it changed.
    pub(crate) fn browser_find_edited(&mut self, edit: Edit) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        let tab = &mut **browser;
        let Some(search) = tab.search.as_mut() else {
            return;
        };
        let before = search.query.text().to_owned();
        search.query.apply(edit);
        if search.query.text() != before {
            search.counted = false;
            tab.commands.push(Command::Find {
                text: search.query.text().to_owned(),
                forward: true,
                again: false,
            });
        }
    }

    /// Takes in the count the engine reported for the page of tab `tab`: which match is current and how many there are.
    pub fn browser_found(&mut self, tab: u32, active: u32, total: u32) {
        if let Some(search) = self
            .browser
            .as_mut()
            .and_then(|browser| browser.tab_mut(tab))
            .and_then(|tab| tab.search.as_mut())
        {
            search.active = active;
            search.total = total;
            search.counted = true;
        }
    }
}

/// Emits the bar for finding in the page, over the top right corner of the page, which is `page` logical pixels wide and has `beside` logical pixels of the pane to its right; the line the text is typed in narrows to keep the bar inside a narrow page.
///
/// It is placed in the layer that holds the page.
#[track_caller]
pub fn find_bar(search: &PageSearch, page: f32, beside: f32) {
    let (said, tone) = search.tally();
    let field = (page - 2.0 * EDGE - BAR_EXTRA).clamp(FIELD_LEAST, FIELD_WIDTH);
    row(
        Modifier::new()
            .placed(BoxAlignment::TopEnd)
            .at(-sp((EDGE + beside) / 4.0), sp(2))
            .z_index(40)
            .shadows(&SHADOW, Radius::Md)
            .fill_rounded(Role::Surface, Radius::Md)
            .stroke(Role::Border, Radius::Md)
            .target(BROWSER_FIND_BAR)
            .pad(1),
        Flex::new().center().gap(1),
        || {
            block(Modifier::new().width(field), || {
                Input::new(&search.query, BROWSER_FIND_FIELD)
                    .size(Size::Sm)
                    .placeholder("Find in page")
                    .invalid(!search.query.text().is_empty() && search.counted && search.total == 0)
                    .build();
            });
            block(Modifier::new().width(COUNT_WIDTH).pad_x(1), || {
                line(said, 11, 1.4, tone);
            });
            Button::icon(IconName::ArrowUp, BROWSER_FIND_PREVIOUS).build();
            Button::icon(IconName::ArrowDown, BROWSER_FIND_NEXT).build();
            Button::icon(IconName::Close, BROWSER_FIND_CLOSE).build();
        },
    );
}
