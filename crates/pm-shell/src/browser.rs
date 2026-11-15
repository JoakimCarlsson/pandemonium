//! The browser beside a session's work: its tabs, the tab shown, the developer tools beside it, and what the engine has yet to be told for each tab.

use std::sync::atomic::{AtomicU32, Ordering};

use pm_browser::{CaptureKind, Cursor, Frame};

use crate::browser_address::address_of;
use crate::browser_safety::Safety;

/// The most tabs one browser holds.
pub const MOST_TABS: usize = 32;

/// Where the browser opens when it is opened with nothing typed.
pub const HOME: &str = "http://localhost:5173";

/// How wide the developer tools are until they are dragged, in logical pixels.
pub const TOOLS_WIDTH: f32 = 500.0;

/// How narrow the developer tools, and the page beside them, can be made, in logical pixels.
pub const TOOLS_LEAST: f32 = 200.0;

/// The number the next tab of any browser in the window gets.
static NEXT_TAB: AtomicU32 = AtomicU32::new(1);

/// A tab number no other tab of the window has had, so the pages of every session's tabs can be told apart by number.
pub fn next_tab_number() -> u32 {
    NEXT_TAB.fetch_add(1, Ordering::Relaxed)
}

/// How a zoom command changes the zoom of the page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomStep {
    /// To the next larger zoom.
    In,
    /// To the next smaller zoom.
    Out,
    /// Back to 100%.
    Reset,
}

/// What the engine is to do next for a tab.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Navigate to the address.
    Load(String),
    /// Go back one page.
    Back,
    /// Go forward one page.
    Forward,
    /// Load the page again.
    Reload,
    /// Load the page again without using the cache.
    HardReload,
    /// Cut what is marked in the page.
    Cut,
    /// Copy what is marked in the page.
    Copy,
    /// Paste the clipboard into the page.
    Paste,
    /// Mark everything in the page.
    SelectAll,
    /// Open the developer tools of the page.
    OpenTools,
    /// Close the developer tools.
    CloseTools,
    /// Look for `text` in the page; `again` steps to the next match, forward or back, of the text already looked for.
    Find {
        /// What to look for.
        text: String,
        /// Whether to go forward from the current match.
        forward: bool,
        /// Whether this steps to the next match of the same text, not a new search.
        again: bool,
    },
    /// End the search in the page and clear its marks.
    StopFind,
    /// Change the zoom of the page.
    Zoom(ZoomStep),
    /// Open the page's address in the system's browser.
    OpenExternal,
    /// Throw away the cookies, site storage and cache of the browser, then load the page again.
    ClearData,
    /// Turn on or off the mode in which the reader picks an element of the page for the prompt.
    Pick(bool),
    /// Take a screenshot of the page, or read its console, for the prompt.
    Capture(CaptureKind),
}

/// A picture a page painted, compared by which paint it is rather than by its pixels.
#[derive(Clone, Debug)]
pub struct Picture(pub Frame);

impl PartialEq for Picture {
    /// Two pictures are the same when they are the same paint of the same size.
    fn eq(&self, other: &Self) -> bool {
        self.0.generation == other.0.generation
            && self.0.width == other.0.width
            && self.0.height == other.0.height
            && std::sync::Arc::ptr_eq(&self.0.rgba, &other.0.rgba)
    }
}

/// The search of a page while its bar shows: what is looked for and what the engine found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageSearch {
    /// What is looked for.
    pub query: String,
    /// Which match is the current one, counting from 1, and 0 for none.
    pub active: u32,
    /// How many matches the engine found.
    pub total: u32,
    /// Whether the engine has counted the matches of the query as it stands.
    pub counted: bool,
}

impl PageSearch {
    /// What the bar says of the matches: nothing before they are counted, that there are none, or which is current of how many; and whether that is a failure.
    pub fn tally(&self) -> (String, bool) {
        if self.query.is_empty() || !self.counted {
            (String::new(), false)
        } else if self.total == 0 {
            ("No results".to_owned(), true)
        } else {
            (format!("{} of {}", self.active, self.total), false)
        }
    }
}

/// The menu open over a page, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserMenu {
    /// Where it opened, in logical pixels from the page's corner.
    pub at: (f32, f32),
}

/// One tab of the browser: its page, address and zoom, and what the reader and the agents may do to it.
#[derive(Clone, Debug, PartialEq)]
pub struct BrowserTab {
    /// The number the tab keeps for as long as it lives.
    pub id: u32,
    /// The address in its address bar.
    pub address: String,
    /// Whether the reader is typing in the address bar, so the page's own address does not overwrite it.
    pub editing: bool,
    /// The title of the page.
    pub title: String,
    /// The newest picture of the page.
    pub page: Option<Picture>,
    /// How the pointer looks over the page.
    pub pointer: Cursor,
    /// The machine the page is on when it is not this one, as the project that opened it names it.
    pub host: Option<String>,
    /// Why the page cannot be shown, when it cannot.
    pub notice: Option<String>,
    /// The search of the page while its bar shows.
    pub search: Option<PageSearch>,
    /// The zoom of the page as a factor, 1.0 being 100%.
    pub zoom: f32,
    /// What the page asked of the reader, a site whose certificate is not trusted, and the last download.
    pub safety: Safety,
    /// What the engine has not yet been told to do for this tab.
    pub commands: Vec<Command>,
    /// Whether an agent opened the tab.
    pub agent: bool,
    /// Whether the tab is kept from the agents' tools that change a page; never so for a tab an agent opened.
    pub locked: bool,
    /// Whether the reader is picking an element of the page for the prompt.
    pub picking: bool,
}

impl BrowserTab {
    /// A tab numbered `id` about to load `address` on `host`, opened by an agent when `agent`.
    pub fn new(id: u32, address: &str, host: Option<String>, agent: bool) -> Self {
        Self {
            id,
            address: address.to_owned(),
            editing: false,
            title: String::new(),
            page: None,
            pointer: Cursor::Arrow,
            host,
            notice: None,
            search: None,
            zoom: 1.0,
            safety: Safety::default(),
            commands: vec![Command::Load(address.to_owned())],
            agent,
            locked: false,
            picking: false,
        }
    }

    /// What the strip calls the tab: its title, or its address while it has none.
    pub fn label(&self) -> String {
        let title = self.title.trim();
        if !title.is_empty() {
            return title.to_owned();
        }
        let address = self.address.trim();
        if address.is_empty() || address == "about:blank" {
            "New tab".to_owned()
        } else {
            address.to_owned()
        }
    }

    /// Loads `address` on `host`, as typing it and pressing Enter does.
    pub fn load(&mut self, address: &str, host: Option<String>) {
        self.host = host;
        address.clone_into(&mut self.address);
        self.editing = false;
        self.commands.push(Command::Load(address.to_owned()));
    }

    /// Shows the bar for finding in the page and looks for the text looked for last again.
    pub fn find_opened(&mut self) {
        let search = self.search.get_or_insert_with(PageSearch::default);
        if !search.query.is_empty() {
            search.counted = false;
            let text = search.query.clone();
            self.commands.push(Command::Find {
                text,
                forward: true,
                again: false,
            });
        }
    }

    /// Looks for `query` in the page as it was typed in the bar.
    pub fn find_typed(&mut self, query: String) {
        let Some(search) = self.search.as_mut() else {
            return;
        };
        if search.query == query {
            return;
        }
        search.query = query.clone();
        search.counted = false;
        if query.is_empty() {
            self.commands.push(Command::StopFind);
        } else {
            self.commands.push(Command::Find {
                text: query,
                forward: true,
                again: false,
            });
        }
    }

    /// Goes to the next match, or the previous one when `back`; shows the bar when it is not shown.
    pub fn find_stepped(&mut self, back: bool) {
        let Some(search) = self.search.as_ref() else {
            self.find_opened();
            return;
        };
        if !search.query.is_empty() {
            let text = search.query.clone();
            self.commands.push(Command::Find {
                text,
                forward: !back,
                again: true,
            });
        }
    }

    /// Hides the bar for finding and clears the marks in the page.
    pub fn find_closed(&mut self) {
        if self.search.take().is_some() {
            self.commands.push(Command::StopFind);
        }
    }

    /// Takes in the count the engine reported: which match is current and how many there are.
    pub fn found(&mut self, active: u32, total: u32) {
        if let Some(search) = self.search.as_mut() {
            search.active = active;
            search.total = total;
            search.counted = true;
        }
    }
}

/// A tab as it is kept across a restart.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SavedTab {
    /// The address in its address bar.
    pub address: String,
    /// The machine the page was on, when it was not this one.
    pub host: Option<String>,
    /// Whether an agent opened it.
    pub agent: bool,
    /// Whether it was locked against agents.
    pub locked: bool,
}

/// What a tab showing `current` does when a server starts serving `address`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Move {
    /// It is on another page, which the reader or an agent is using, and is left there; the page opens in a tab of its own.
    NewTab,
    /// It shows nothing or an error, and loads the page.
    Load,
    /// It shows a page of the same server, which started again, and loads that page again where it is.
    Reload,
}

/// `address` with a loopback host written as `localhost`, so a page opened by one name of this machine is the page of the others.
fn loopback_as_localhost(address: &str) -> String {
    let address = address.trim();
    let Some((scheme, rest)) = address.split_once("://") else {
        return address.to_owned();
    };
    for name in ["127.0.0.1", "[::1]"] {
        if let Some(after) = rest.strip_prefix(name)
            && (after.is_empty() || after.starts_with([':', '/', '?', '#']))
        {
            return format!("{scheme}://localhost{after}");
        }
    }
    address.to_owned()
}

/// What a tab showing `current` does when a server starts serving `address`.
pub(crate) fn served_move(current: &str, address: &str) -> Move {
    let current = current.trim();
    if current.is_empty() || current == "about:blank" || current.starts_with("chrome-error:") {
        return Move::Load;
    }
    let current = loopback_as_localhost(current);
    let origin = loopback_as_localhost(address);
    let origin = origin.trim_end_matches('/');
    let same = current
        .strip_prefix(origin)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?', '#']));
    if same { Move::Reload } else { Move::NewTab }
}

/// A session's browser: its tabs, the developer tools beside the page of the tab shown, and what is shared by every tab.
#[derive(Clone, Debug, PartialEq)]
pub struct Browser {
    /// The tabs, in the order of the strip; never empty.
    pub tabs: Vec<BrowserTab>,
    /// The index of the tab shown.
    pub active: usize,
    /// The tab the agents' tools work on when they name none, when an agent chose one.
    pub agent_tab: Option<u32>,
    /// Whether the developer tools are open beside the page.
    pub tools: bool,
    /// How wide the developer tools are when open, in logical pixels.
    pub tools_width: f32,
    /// The newest picture of the developer tools.
    pub tools_page: Option<Picture>,
    /// Whether the question about clearing cookies and site data is open.
    pub clearing: bool,
    /// The menu open over the page, if one is.
    pub menu: Option<BrowserMenu>,
}

impl Browser {
    /// A browser with one tab about to load `address` on `host`.
    pub fn new(address: &str, host: Option<String>) -> Self {
        Self {
            tabs: vec![BrowserTab::new(next_tab_number(), address, host, false)],
            active: 0,
            agent_tab: None,
            tools: false,
            tools_width: TOOLS_WIDTH,
            tools_page: None,
            clearing: false,
            menu: None,
        }
    }

    /// A browser as it was left: with `tabs`, the one at `active` shown, the developer tools open when `tools`, `tools_width` wide; nothing when `tabs` is empty.
    pub fn restored(
        tabs: &[SavedTab],
        active: usize,
        tools: bool,
        tools_width: f32,
    ) -> Option<Self> {
        let first = tabs.first()?;
        let mut browser = Self::new(&first.address, first.host.clone());
        for saved in &tabs[1..] {
            browser.open_tab(&saved.address, saved.host.clone(), saved.agent, false);
        }
        for (tab, saved) in browser.tabs.iter_mut().zip(tabs) {
            tab.agent = saved.agent;
            tab.locked = saved.locked && !saved.agent;
        }
        browser.active = active.min(browser.tabs.len() - 1);
        if tools_width > 0.0 {
            browser.tools_width = tools_width;
        }
        if tools {
            browser.tools = true;
            let shown = browser.active;
            browser.tabs[shown].commands.push(Command::OpenTools);
        }
        Some(browser)
    }

    /// The tab shown.
    pub fn shown(&self) -> &BrowserTab {
        &self.tabs[self.active]
    }

    /// The tab shown, to change.
    pub fn shown_mut(&mut self) -> &mut BrowserTab {
        &mut self.tabs[self.active]
    }

    /// The number of the tab shown.
    pub fn active_id(&self) -> u32 {
        self.shown().id
    }

    /// The index of the tab numbered `id`.
    pub fn tab_index(&self, id: u32) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == id)
    }

    /// The tab numbered `id`.
    pub fn tab(&self, id: u32) -> Option<&BrowserTab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    /// The tab numbered `id`, to change.
    pub fn tab_mut(&mut self, id: u32) -> Option<&mut BrowserTab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    /// Opens a tab on `address`, giving its number, or nothing when the browser holds as many tabs as it may; it is shown when `show`, and otherwise opens behind the one shown.
    pub fn open_tab(
        &mut self,
        address: &str,
        host: Option<String>,
        agent: bool,
        show: bool,
    ) -> Option<u32> {
        if self.tabs.len() >= MOST_TABS {
            return None;
        }
        let id = next_tab_number();
        self.tabs.push(BrowserTab::new(id, address, host, agent));
        if agent {
            self.agent_tab = Some(id);
        }
        if show {
            self.select(self.tabs.len() - 1);
        }
        Some(id)
    }

    /// Shows the tab at `index`; the developer tools, when open, move to it.
    pub fn select(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.active {
            return;
        }
        self.menu = None;
        self.active = index;
        self.follow_tools();
    }

    /// Opens the developer tools on the tab shown when they are open, and so closes them on the one left.
    fn follow_tools(&mut self) {
        if self.tools {
            self.tools_page = None;
            let active = self.active;
            self.tabs[active].commands.push(Command::OpenTools);
        }
    }

    /// Closes the tab at `index` and says whether it was the last, which leaves the browser with nothing to show.
    pub fn close_tab(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }
        let removed = self.tabs.remove(index);
        if self.agent_tab == Some(removed.id) {
            self.agent_tab = None;
        }
        if self.tabs.is_empty() {
            return true;
        }
        if index < self.active {
            self.active -= 1;
        } else if index == self.active {
            self.active = index.min(self.tabs.len() - 1);
            self.follow_tools();
        }
        false
    }

    /// Shows or hides the developer tools beside the page.
    pub fn toggle_tools(&mut self) {
        self.tools = !self.tools;
        self.tools_page = None;
        let command = if self.tools {
            Command::OpenTools
        } else {
            Command::CloseTools
        };
        self.shown_mut().commands.push(command);
    }

    /// Shows the developer tools when they are hidden.
    pub fn show_tools(&mut self) {
        if !self.tools {
            self.toggle_tools();
        }
    }

    /// Queues `command` for the tab shown.
    pub fn queue(&mut self, command: Command) {
        self.shown_mut().commands.push(command);
    }

    /// Loads what was typed in the address bar of the tab shown, on `host`.
    pub fn go(&mut self, host: Option<String>) {
        let address = address_of(&self.shown().address);
        self.shown_mut().load(&address, host);
    }

    /// Shows the page a program serves at `address` on `host`: a tab on a page of the same server loads it again where it is; failing that the tab shown loads it when it is on nothing or an error, and otherwise it opens in a new tab behind the one shown.
    pub fn serve(&mut self, address: &str, host: Option<String>) {
        let again: Vec<usize> = (0..self.tabs.len())
            .filter(|index| {
                self.tabs[*index].host == host
                    && served_move(&self.tabs[*index].address, address) == Move::Reload
            })
            .collect();
        if !again.is_empty() {
            for index in again {
                self.tabs[index].commands.push(Command::Reload);
            }
        } else if served_move(&self.shown().address, address) == Move::Load {
            self.shown_mut().load(address, host);
        } else {
            self.open_tab(address, host, false, false);
        }
    }

    /// The tab the agents' tools work on when they name none: the one an agent chose or opened, while it exists, and otherwise the one shown.
    pub fn agent_target(&self) -> u32 {
        self.agent_tab
            .filter(|id| self.tab(*id).is_some())
            .unwrap_or_else(|| self.active_id())
    }

    /// What the engine has not yet been told to do, each with the number of the tab it is for.
    pub fn take_commands(&mut self) -> Vec<(u32, Command)> {
        self.tabs
            .iter_mut()
            .flat_map(|tab| {
                let id = tab.id;
                std::mem::take(&mut tab.commands)
                    .into_iter()
                    .map(move |command| (id, command))
            })
            .collect()
    }

    /// The tabs as they are kept across a restart.
    pub fn saved_tabs(&self) -> Vec<SavedTab> {
        self.tabs
            .iter()
            .map(|tab| SavedTab {
                address: tab.address.clone(),
                host: tab.host.clone(),
                agent: tab.agent,
                locked: tab.locked,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
