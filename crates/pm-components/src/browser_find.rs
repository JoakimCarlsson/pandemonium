//! Finding text in the page of the browser: the search the engine runs, and the bar over the top right corner of the page that drives it.

use crate::areas::BrowserEvent;
use crate::theme::Frame;
use crate::theme::ShadowLayers;
use rustle::{Alignment, Arrangement};

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::TextFieldState;
use rustle::phosphor;
use rustle::{BoxAlignment, Modifier};

use crate::events::Interactive;
use crate::theme::{Radius, Role, sp};

use crate::browser::Command;
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
    pub query: TextFieldState,
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
        let search = browser.search.get_or_insert_with(PageSearch::default);
        search.query.select_all();
        if !search.query.text().is_empty() {
            self.browser_find_typed();
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

    /// Looks for the text looked for after the reader changed it.
    pub(crate) fn browser_find_typed(&mut self) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        let tab = &mut **browser;
        let Some(search) = tab.search.as_mut() else {
            return;
        };
        search.counted = false;
        tab.commands.push(Command::Find {
            text: search.query.text(),
            forward: true,
            again: false,
        });
    }

    /// Takes in the count the engine reported for the page of tab `tab`: which match is current and how many there are.
    pub fn browser_found(&mut self, tab: u32, active: u32, total: u32) {
        if let Some(search) = self.browser_tab(tab).and_then(|tab| tab.search.as_mut()) {
            search.active = active;
            search.total = total;
            search.counted = true;
        }
    }
}

/// Emits the bar for finding in the page, over the top right corner of the page, which is `page` logical pixels wide and has `beside` logical pixels of the pane to its right; the line the text is typed in narrows to keep the bar inside a narrow page.
///
/// It is placed in the layer that holds the page. `on_hover` is told when the pointer comes
/// over the bar and when it leaves it, as the page under it is not under the pointer then.
#[track_caller]
pub fn find_bar(search: &PageSearch, page: f32, beside: f32, on_hover: impl Fn(bool) + 'static) {
    let (said, tone) = search.tally();
    let field = (page - 2.0 * EDGE - BAR_EXTRA).clamp(FIELD_LEAST, FIELD_WIDTH);
    rustle::row_arranged(
        Modifier::new()
            .align(BoxAlignment::TopEnd)
            .offset(-sp((EDGE + beside) / 4.0), sp(2))
            .z_index(40)
            .shadow_layers(&SHADOW, Radius::Md.px())
            .panel(Radius::Md)
            .on_hover(on_hover)
            .control(
                Target::Browser(BrowserEvent::BrowserFindBar),
                send(Event::Browser(BrowserEvent::BrowserFindBar)),
            )
            .padding(theme::sp(1)),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            rustle::column_arranged(
                Modifier::new().width(field),
                Arrangement::Start,
                Alignment::Start,
                || {
                    Input::new(
                        &search.query,
                        Target::Browser(BrowserEvent::BrowserFindField),
                    )
                    .modifier(Modifier::new().fill_max_width())
                    .size(Size::Sm)
                    .placeholder("Find in page")
                    .invalid(!search.query.text().is_empty() && search.counted && search.total == 0)
                    .submits(Target::Browser(BrowserEvent::BrowserFindNext))
                    .build();
                },
            );
            rustle::column_arranged(
                Modifier::new()
                    .width(COUNT_WIDTH)
                    .padding_xy(theme::sp(1), 0.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    line(said, 11, 1.4, tone);
                },
            );
            for (icon, id) in [
                (phosphor::ARROW_UP, BrowserEvent::BrowserFindPrevious),
                (phosphor::ARROW_DOWN, BrowserEvent::BrowserFindNext),
                (phosphor::X, BrowserEvent::BrowserFindClose),
            ] {
                Button::icon(icon, Target::Browser(id)).build();
            }
        },
    );
}
