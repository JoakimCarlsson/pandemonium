//! The tabs of the browser: what each tab holds, opening, showing and closing them, the lock that keeps agents out of a tab, and the strip that draws them.

use std::ops::Range;

use pm_input::FocusId;
use pm_style::{
    Role, bg, border_b, border_color, flex, flex_none, gap, h_px, items_center, overflow_hidden,
    px, w_full,
};
use pm_text::Field;
use pm_ui::{Node, Picture, node};

use crate::Button;
use crate::browser::{Browser, Command, Pointer};
use crate::browser_find::PageSearch;
use crate::browser_safety::Safety;
use crate::button::Variant;
use crate::size::Size;
use crate::state::{Tab, WorkspaceState};
use crate::tab_chip::{TabChip, TabVariant};
use pm_icon::IconName;

/// Opens a new, empty tab.
pub const BROWSER_NEW_TAB: FocusId = FocusId(7_440_022);

/// Locks the tab being shown against agents, or unlocks it.
pub const BROWSER_LOCK: FocusId = FocusId(7_440_023);

/// Scrolls the strip of tabs back to the tabs before the first one shown.
pub const BROWSER_TABS_PREVIOUS: FocusId = FocusId(7_440_024);

/// Scrolls the strip of tabs on to the tabs after the last one shown.
pub const BROWSER_TABS_NEXT: FocusId = FocusId(7_440_025);

/// The first target of a tab; the tab at index `i` is `BROWSER_TAB_SELECT + i`.
pub const BROWSER_TAB_SELECT: u64 = 7_440_100;

/// The first target of a tab's close button; the tab at index `i` is `BROWSER_TAB_CLOSE + i`.
pub const BROWSER_TAB_CLOSE: u64 = 7_440_200;

/// The most tabs the browser holds.
pub const MOST_TABS: usize = 32;

/// How tall the strip of tabs is, in logical pixels.
pub const TAB_STRIP: f32 = 34.0;

/// How wide a tab is when there is room, in logical pixels.
const TAB_WIDTH: f32 = 168.0;

/// How narrow a tab is made before the strip scrolls instead, in logical pixels.
const TAB_LEAST: f32 = 84.0;

/// How much room the buttons at the ends of the strip take, in logical pixels.
const STRIP_BUTTONS: f32 = 30.0;

/// The space the strip keeps at its two ends, and the gaps between what it holds, taken from the width of the tabs, in logical pixels.
const STRIP_PADDING: f32 = 16.0;

/// The gap after each tab, in logical pixels.
const TAB_GAP: f32 = 4.0;

/// What a tool of an agent that reads a page, and so is allowed on a locked tab, is called.
const READS: [&str; 5] = [
    "browser_snapshot",
    "browser_screenshot",
    "browser_console",
    "browser_get_bounding_box",
    "browser_wait",
];

/// Whether the tool `tool` may change a page: every tool does except the ones that only read it, and a tool nobody classified counts as changing it, so a new tool is kept out of a locked tab until it is listed.
pub fn changes_page(tool: &str) -> bool {
    !(READS.contains(&tool) || tool.starts_with("browser_network"))
}

/// What a tool of an agent says when it is kept out of a locked tab.
pub fn locked_message(tool: &str, tab: u32) -> String {
    format!(
        "Tab {tab} is locked by the person using the browser, so {tool} cannot change it. Open a tab of your own with browser_tabs (action \"new\") and work there; reading the locked tab with browser_snapshot, browser_screenshot, browser_console and browser_network is still allowed."
    )
}

/// One tab of the browser: its page, address, history and zoom, and what the reader and the agents may do to it.
#[derive(Clone, Debug, PartialEq)]
pub struct BrowserTab {
    /// The number the tab keeps for as long as it lives, which agents name it by.
    pub id: u32,
    /// The line the address is typed in.
    pub address: Field,
    /// The title of the page.
    pub title: String,
    /// The newest picture of the page.
    pub page: Option<Picture>,
    /// How the pointer looks over the page.
    pub pointer: Pointer,
    /// Whether a click on the page picks the element under the pointer.
    pub picking: bool,
    /// Whether the reader is dragging a rectangle over the page to add a picture of it.
    pub picking_area: bool,
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
    /// Whether the tab is kept from the agents' tools that change a page; never so for a tab an agent opened, which agents always may work in.
    pub locked: bool,
}

impl BrowserTab {
    /// A tab about to load `address`, opened by an agent when `agent`.
    pub(crate) fn new(id: u32, address: &str, host: Option<String>, agent: bool) -> Self {
        Self {
            id,
            address: Field::new(address),
            title: String::new(),
            page: None,
            pointer: Pointer::Arrow,
            picking: false,
            picking_area: false,
            host,
            notice: None,
            search: None,
            zoom: 1.0,
            safety: Safety::default(),
            commands: vec![Command::Load(address.to_owned())],
            agent,
            locked: false,
        }
    }

    /// What the strip calls the tab.
    pub fn label(&self) -> String {
        let title = self.title.trim();
        if !title.is_empty() {
            return title.to_owned();
        }
        let address = self.address.text().trim();
        if address.is_empty() || address == "about:blank" {
            "New tab".to_owned()
        } else {
            address.to_owned()
        }
    }
}

/// A tab as it is kept across a restart.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedTab {
    /// The address in its address line.
    pub address: String,
    /// The machine the page was on, when it was not this one.
    pub host: Option<String>,
    /// Whether an agent opened it.
    pub agent: bool,
    /// Whether it was locked against agents.
    pub locked: bool,
}

/// What an agent is told of a tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabInfo {
    /// The tab's number.
    pub id: u32,
    /// What the strip calls it.
    pub title: String,
    /// The address in its address line.
    pub address: String,
    /// Whether an agent opened it.
    pub agent: bool,
    /// Whether it is locked against agents.
    pub locked: bool,
    /// Whether the reader is looking at it.
    pub viewing: bool,
    /// Whether it is the tab the agents' tools work on when they name none.
    pub target: bool,
}

impl Browser {
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

    /// The number of the tab being shown.
    pub fn active_id(&self) -> u32 {
        self.tabs[self.active].id
    }

    /// Opens a tab on `address`, giving its number, or nothing when the browser holds as many tabs as it may; it is shown when `show`, and otherwise opens behind the one being shown.
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
        let id = self.next_tab;
        self.next_tab += 1;
        self.tabs.push(BrowserTab::new(id, address, host, agent));
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
        self.end_modes();
        self.active = index;
        self.show_in_strip();
        self.follow_tools();
    }

    /// Scrolls the strip, when it does not draw every tab, so that the tab being shown is drawn.
    fn show_in_strip(&mut self) {
        let fit = self.strip_fit.get();
        if self.active < self.strip_start {
            self.strip_start = self.active;
        } else if fit != usize::MAX && self.active >= self.strip_start + fit {
            self.strip_start = self.active + 1 - fit;
        }
    }

    /// Ends the modes of picking in the tab being shown, which cannot go on in a page that is no longer shown.
    fn end_modes(&mut self) {
        let tab = &mut self.tabs[self.active];
        if tab.picking {
            tab.picking = false;
            tab.commands.push(Command::Pick(false));
        }
        if tab.picking_area {
            tab.picking_area = false;
            tab.commands.push(Command::PickArea(false));
        }
    }

    /// Opens the developer tools on the tab being shown when they are open, and so closes them on the one left.
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
        self.strip_start = self.strip_start.min(self.tabs.len() - 1);
        self.show_in_strip();
        false
    }

    /// The range of tabs the strip draws in `width` logical pixels, and how wide each is: all of them when each can be `TAB_LEAST` wide, and otherwise as many as fit, from the one the strip was scrolled to (showing a tab scrolls the strip to it).
    pub fn strip_window(&self, width: f32) -> (Range<usize>, f32) {
        let count = self.tabs.len();
        let room = (width - STRIP_BUTTONS - STRIP_PADDING).max(TAB_LEAST);
        let each = |room: f32, fit: usize| (room / fit as f32).floor() - TAB_GAP;
        if count as f32 * (TAB_LEAST + TAB_GAP) <= room {
            return (0..count, each(room, count).min(TAB_WIDTH));
        }
        let room = room - 2.0 * STRIP_BUTTONS;
        let fit = ((room / (TAB_LEAST + TAB_GAP)).floor().max(1.0) as usize).min(count);
        self.strip_fit.set(fit);
        let start = self.strip_start.min(count - fit);
        (start..start + fit, each(room, fit).min(TAB_WIDTH))
    }

    /// What agents are told of each tab, in the order of the strip.
    pub fn tab_infos(&self) -> Vec<TabInfo> {
        let target = self.agent_target();
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| TabInfo {
                id: tab.id,
                title: tab.label(),
                address: tab.address.text().to_owned(),
                agent: tab.agent,
                locked: tab.locked,
                viewing: index == self.active,
                target: tab.id == target,
            })
            .collect()
    }

    /// The tab the agents' tools work on when they name none: the one an agent chose or opened, while it exists, and otherwise the one being shown.
    pub fn agent_target(&self) -> u32 {
        self.agent_tab
            .filter(|id| self.tab(*id).is_some())
            .unwrap_or_else(|| self.active_id())
    }
}

impl WorkspaceState {
    /// Opens the browser if it is not open, then a tab on `address` for an agent when `agent`, shown when `show`; gives the tab's number, or nothing when there are too many tabs.
    pub fn browser_tab_opened(
        &mut self,
        address: &str,
        host: Option<String>,
        agent: bool,
        show: bool,
    ) -> Option<u32> {
        let Some(browser) = self.browser.as_mut() else {
            let mut browser = Browser::new(address, host);
            browser.tabs[0].agent = agent;
            browser.agent_tab = agent.then(|| browser.tabs[0].id);
            let id = browser.tabs[0].id;
            self.browser = Some(browser);
            self.tab = Tab::Browser;
            return Some(id);
        };
        let id = browser.open_tab(address, host, agent, show)?;
        if agent {
            browser.agent_tab = Some(id);
        }
        Some(id)
    }

    /// Shows the browser pane, as opening a tab does.
    pub fn browser_pane_shown(&mut self) {
        self.tab = Tab::Browser;
    }

    /// Closes the tab numbered `id`, and the browser with it when it was the last; says whether there was such a tab.
    pub fn browser_tab_closed(&mut self, id: u32) -> bool {
        let Some(browser) = self.browser.as_mut() else {
            return false;
        };
        let Some(index) = browser.tab_index(id) else {
            return false;
        };
        if browser.close_tab(index) {
            self.browser_dropped();
        }
        true
    }

    /// Carries out pressing a target of the tab strip or the lock, and says whether `id` was one.
    pub(crate) fn browser_tab_pressed(&mut self, id: FocusId) -> bool {
        let host = self.current_project_host();
        let Some(browser) = self.browser.as_mut() else {
            return false;
        };
        let count = browser.tabs.len() as u64;
        match id {
            BROWSER_NEW_TAB => {
                browser.open_tab("about:blank", host, false, true);
            }
            BROWSER_LOCK => {
                let tab = &mut browser.tabs[browser.active];
                tab.locked = !tab.locked && !tab.agent;
            }
            BROWSER_TABS_PREVIOUS => {
                browser.strip_start = browser.strip_start.saturating_sub(1);
            }
            BROWSER_TABS_NEXT => {
                browser.strip_start = (browser.strip_start + 1).min(browser.tabs.len() - 1);
            }
            FocusId(raw) if (BROWSER_TAB_SELECT..BROWSER_TAB_SELECT + count).contains(&raw) => {
                browser.select((raw - BROWSER_TAB_SELECT) as usize);
                self.tab = Tab::Browser;
            }
            FocusId(raw) if (BROWSER_TAB_CLOSE..BROWSER_TAB_CLOSE + count).contains(&raw) => {
                let index = (raw - BROWSER_TAB_CLOSE) as usize;
                if browser.close_tab(index) {
                    self.browser_dropped();
                }
            }
            _ => return false,
        }
        true
    }

    /// Opens a new tab and shows it, as the key for it does.
    pub fn browser_new_tab(&mut self) {
        self.browser_tab_pressed(BROWSER_NEW_TAB);
    }

    /// Closes the tab being shown, as the key for it does.
    pub fn browser_close_tab(&mut self) {
        let Some(browser) = self.browser.as_ref() else {
            return;
        };
        let id = FocusId(BROWSER_TAB_CLOSE + browser.active as u64);
        self.browser_tab_pressed(id);
    }

    /// Locks the tab being shown against agents, or unlocks it, as the padlock does.
    pub fn browser_lock_toggled(&mut self) {
        self.browser_tab_pressed(BROWSER_LOCK);
    }
}

/// The strip of tabs, `width` logical pixels wide: a chip for each tab that fits, buttons to scroll when not all do, and the one that opens a new tab.
pub fn tab_strip(browser: &Browser, width: f32) -> Node {
    let (shown, each) = browser.strip_window(width);
    let scrolls = shown.len() < browser.tabs.len();
    let mut strip = node()
        .styles([
            flex(),
            items_center(),
            gap(1),
            px(2),
            h_px(TAB_STRIP),
            w_full(),
            flex_none(),
            overflow_hidden(),
            border_b(),
            border_color(Role::Border),
            bg(Role::Sidebar),
        ])
        .children(scrolls.then(|| {
            Button::icon(IconName::ArrowLeft, BROWSER_TABS_PREVIOUS)
                .variant(Variant::Ghost)
                .size(Size::Xs)
                .disabled(shown.start == 0)
                .build()
        }));
    for index in shown.clone() {
        let tab = &browser.tabs[index];
        let hint = if tab.agent {
            format!("{} (opened by an agent)", tab.label())
        } else {
            tab.label()
        };
        strip = strip.child(
            TabChip::new(
                tab.label(),
                FocusId(BROWSER_TAB_SELECT + index as u64),
                FocusId(BROWSER_TAB_CLOSE + index as u64),
            )
            .variant(if tab.agent {
                TabVariant::Agent
            } else {
                TabVariant::Page
            })
            .selected(index == browser.active)
            .locked(tab.locked)
            .width(each)
            .hint(hint)
            .build(),
        );
    }
    strip
        .children(scrolls.then(|| {
            Button::icon(IconName::ArrowRight, BROWSER_TABS_NEXT)
                .variant(Variant::Ghost)
                .size(Size::Xs)
                .disabled(shown.end == browser.tabs.len())
                .build()
        }))
        .child(
            Button::icon(IconName::Plus, BROWSER_NEW_TAB)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build(),
        )
}

#[cfg(test)]
mod tests;
