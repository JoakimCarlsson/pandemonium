//! The browser pane: a page of the web beside the agent's work, with its address bar and, beside it, the developer tools of the page.

use crate::areas::BrowserEvent;
use crate::button;
use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::Modifier;
use rustle::TextFieldState;
use rustle::phosphor;
use rustle::{Alignment, Arrangement};

use std::cell::Cell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::events::Interactive;
use crate::icons::panel_right;
use crate::theme::Role;

use crate::browser_address::address_of;
use crate::browser_find::find_bar;
use crate::browser_pointing::{PagePointer, cursor_of};
use crate::browser_safety::{Answer, certificate_page};
use crate::browser_tabs::{BrowserTab, SavedTab, tab_strip};
use crate::button::Variant;
use crate::parts::line;
use crate::sash::Sash;
use crate::size::Size;
use crate::state::{Page, Tab, WorkspaceState};
use crate::{Button, Input};

/// Where a page opens when the browser is opened with nothing typed.
pub const HOME: &str = "http://localhost:5173";

/// How tall the bar above the page is, in logical pixels.
pub const TOOLBAR: f32 = 44.0;

/// How wide the developer tools are until they are dragged, in logical pixels.
const TOOLS_WIDTH: f32 = 500.0;

/// How narrow the developer tools, and the page beside them, can be made, in logical pixels.
pub const TOOLS_LEAST: f32 = 200.0;

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

/// What the window asks the engine to do next.
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
    /// Cut what is marked in the page.
    Cut,
    /// Copy what is marked in the page.
    Copy,
    /// Paste the clipboard into the page.
    Paste,
    /// Mark everything in the page.
    SelectAll,
    /// Turn on or off the mode in which a click picks an element of the page.
    Pick(bool),
    /// Add a picture of what the page shows to the prompt.
    Screenshot,
    /// Add a picture of the whole page to the prompt.
    FullPage,
    /// Add the lines of the page's console to the prompt.
    Console,
    /// Turn on or off the mode in which the reader drags a rectangle over the page to add a picture of it.
    PickArea(bool),
    /// Open the developer tools.
    OpenTools,
    /// Close the developer tools.
    CloseTools,
    /// Load the page again without using the cache.
    HardReload,
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
    /// Show the sealed document the tab holds, in a page of its own that the network is denied to.
    LoadSealed,
}

/// How the pointer looks over the page, as the page asks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Pointer {
    /// The ordinary arrow.
    #[default]
    Arrow,
    /// The hand over a link.
    Hand,
    /// The caret over text.
    Text,
    /// The crosshair.
    Cross,
    /// Busy.
    Wait,
    /// A resize handle running east and west.
    ResizeHorizontal,
    /// A resize handle running north and south.
    ResizeVertical,
}

/// The browser: its tabs, the developer tools beside the page of the tab being shown, and what is shared by every tab.
///
/// It dereferences to the tab being shown, so what concerns the page the reader is looking at is written as it is for a tab.
#[derive(Clone, Debug, PartialEq)]
pub struct Browser {
    /// The tabs, in the order of the strip; never empty while the browser is open.
    pub tabs: Vec<BrowserTab>,
    /// The index of the tab being shown.
    pub active: usize,
    /// The tab the agents' tools work on when they name none, when an agent chose one.
    pub agent_tab: Option<u32>,
    /// The index of the first tab the strip draws when not all fit.
    pub strip_start: usize,
    /// How many tabs the strip drew when last drawn, which keeps the tab shown among them when another is shown.
    pub strip_fit: std::cell::Cell<usize>,
    /// The newest picture of the developer tools.
    pub tools_page: Option<rustle::ImageData>,
    /// Whether the developer tools are open beside the page.
    pub tools: bool,
    /// How wide the developer tools are when open, in logical pixels.
    pub tools_width: f32,
    /// Whether the question about clearing cookies and site data is open.
    pub clearing: bool,
    /// How many times the reader opened the browser or brought it forward: its address
    /// takes the keyboard each time this changes.
    pub asked: u64,
}

impl Deref for Browser {
    type Target = BrowserTab;

    /// The tab being shown.
    fn deref(&self) -> &BrowserTab {
        &self.tabs[self.active]
    }
}

impl DerefMut for Browser {
    /// The tab being shown.
    fn deref_mut(&mut self) -> &mut BrowserTab {
        &mut self.tabs[self.active]
    }
}

impl Browser {
    /// A browser with one tab about to load `address`.
    pub(crate) fn new(address: &str, host: Option<String>) -> Self {
        Self {
            tabs: vec![BrowserTab::new(
                crate::browser_scope::next_tab_number(),
                address,
                host,
                false,
            )],
            active: 0,
            agent_tab: None,
            strip_start: 0,
            strip_fit: std::cell::Cell::new(usize::MAX),
            tools_page: None,
            tools: false,
            tools_width: TOOLS_WIDTH,
            clearing: false,
            asked: 0,
        }
    }

    /// A browser as it was left: with `tabs`, the one at `active` shown, the developer tools open when `tools`, `tools_width` wide; nothing when `tabs` is empty.
    pub(crate) fn restored(
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
            browser.commands.push(Command::OpenTools);
        }
        Some(browser)
    }
}

impl Browser {
    /// Turns on the mode of dragging a rectangle over the page, and turns off the one of picking an element.
    pub(crate) fn start_area(&mut self) {
        if self.picking {
            self.picking = false;
            self.commands.push(Command::Pick(false));
        }
        self.picking_area = true;
        self.commands.push(Command::PickArea(true));
    }

    /// Turns off the mode of dragging a rectangle over the page, if it is on.
    fn end_area(&mut self) {
        if self.picking_area {
            self.picking_area = false;
            self.commands.push(Command::PickArea(false));
        }
    }
}

/// What a tab showing `current` does when a page is found at `address`.
#[derive(Debug, PartialEq, Eq)]
enum Move {
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
            return format!("{scheme}://localhost{}", &rest[name.len()..]);
        }
    }
    address.to_owned()
}

/// What a tab showing `current` does when a page is found at `address`.
fn served_move(current: &str, address: &str) -> Move {
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

/// Whether `id` is something of the browser.
pub fn is_browser_part(id: Target) -> bool {
    matches!(id, Target::Browser(_))
}

impl WorkspaceState {
    /// The machine of the project or the chat the reader is in when it is not this one.
    pub(crate) fn current_project_host(&self) -> Option<String> {
        match self.page {
            Page::History | Page::Extensions => self.projects.first()?.host(),
            _ => self.shown_machine(),
        }
    }

    /// Opens the browser on `address`, or shows it when it is open, and shows it.
    pub fn open_browser(&mut self, address: &str) {
        let host = self.current_project_host();
        if self.browser.is_none() {
            self.browser = Some(Browser::new(address, host));
        }
        self.tab = Tab::Browser;
    }

    /// Shows the page a program started in a shell serves at `address` on the machine `host`: the browser opens on it, or when it is open, a tab that shows a page of the same server, which started again, loads it again where it is; failing that the tab being shown loads it when it is on nothing or on an error, and otherwise it opens in a new tab behind the one being shown, which the reader or an agent is using and which stays as it is.
    pub fn open_served_page(&mut self, address: &str, host: Option<String>) {
        match self.browser.as_mut() {
            Some(browser) => {
                let again: Vec<usize> = (0..browser.tabs.len())
                    .filter(|index| {
                        browser.tabs[*index].host == host
                            && served_move(&browser.tabs[*index].address.text(), address)
                                == Move::Reload
                    })
                    .collect();
                if !again.is_empty() {
                    for index in again {
                        browser.tabs[index].commands.push(Command::Reload);
                    }
                } else if served_move(&browser.address.text(), address) == Move::Load {
                    browser.host = host;
                    browser.address = TextFieldState::new(address);
                    browser.commands.push(Command::Load(address.to_owned()));
                } else if browser.open_tab(address, host, false, false).is_none() {
                    return;
                }
            }
            None => self.browser = Some(Browser::new(address, host)),
        }
        self.tab = Tab::Browser;
    }

    /// Shows the page of a link the reader followed at `address`, on the machine of the project the reader is in: the browser opens on it, or goes to it where it is.
    pub fn open_link_page(&mut self, address: &str) {
        let host = self.current_project_host();
        match self.browser.as_mut() {
            Some(browser) => {
                browser.host = host;
                browser.address = TextFieldState::new(address);
                browser.commands.push(Command::Load(address.to_owned()));
            }
            None => self.browser = Some(Browser::new(address, host)),
        }
        self.tab = Tab::Browser;
    }

    /// Opens the browser as it was left: with `tabs`, the one at `active` shown in it, the developer tools open when `tools`, `tools_width` wide, and the browser shown when `shown`.
    pub fn restore_browser(
        &mut self,
        tabs: &[SavedTab],
        active: usize,
        tools: bool,
        tools_width: f32,
        shown: bool,
    ) {
        let Some(browser) = Browser::restored(tabs, active, tools, tools_width) else {
            return;
        };
        self.browser = Some(browser);
        if shown {
            self.tab = Tab::Browser;
        }
    }

    /// How the browser is to be put back after a restart: its tabs, the index of the one shown, whether its developer tools are open and how wide they are.
    pub fn browser_saved(&self) -> Option<(Vec<SavedTab>, usize, bool, f32)> {
        self.browser
            .as_ref()
            .map(crate::browser_scope::saved_parts)
            .filter(|parts| !parts.0.is_empty())
    }

    /// Shows the developer tools beside the page when they are not shown, as pressing their button does.
    pub fn browser_tools_shown(mut self) -> Self {
        if let Some(browser) = self.browser.as_mut()
            && !browser.tools
        {
            browser.tools = true;
            browser.commands.push(Command::OpenTools);
        }
        self
    }

    /// Closes the browser.
    pub fn browser_closed(mut self) -> Self {
        self.browser_dropped();
        self
    }

    /// Takes the browser away, and shows the chat when the browser was what was shown.
    pub(crate) fn browser_dropped(&mut self) {
        self.browser = None;
        if self.tab == Tab::Browser {
            self.tab = Tab::Chat;
        }
    }

    /// Carries out what pressing `id` of the browser does.
    pub(crate) fn browser_pressed(mut self, id: Target) -> Self {
        if self.browser_tab_pressed(id) {
            return self;
        }
        match id {
            Target::Browser(BrowserEvent::BrowserClose) => return self.browser_closed(),
            Target::Browser(BrowserEvent::BrowserFindNext) => self.browser_find_stepped(false),
            Target::Browser(BrowserEvent::BrowserFindPrevious) => self.browser_find_stepped(true),
            Target::Browser(BrowserEvent::BrowserFindClose) => self.browser_find_closed(),
            Target::Browser(BrowserEvent::BrowserClearConfirm) => self.browser_clear_confirmed(),
            Target::Browser(BrowserEvent::BrowserClearCancel)
            | Target::Browser(BrowserEvent::BrowserClearOutside) => {
                self.browser_clear_declined();
            }
            _ => self.browser_page_pressed(id),
        }
        self
    }

    /// Carries out what pressing `id` of the bar above the page, or of a question or note over it, does.
    fn browser_page_pressed(&mut self, id: Target) {
        let host = self.current_project_host();
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        if browser.safety.press(id) {
            return;
        }
        match id {
            Target::Browser(BrowserEvent::TabBrowser) => self.tab = Tab::Browser,
            Target::Browser(BrowserEvent::BrowserBack) => browser.commands.push(Command::Back),
            Target::Browser(BrowserEvent::BrowserForward) => {
                browser.commands.push(Command::Forward)
            }
            Target::Browser(BrowserEvent::BrowserReload) => browser.commands.push(Command::Reload),
            Target::Browser(BrowserEvent::BrowserGo) => {
                let address = address_of(&browser.address.text());
                browser.host = host;
                browser.commands.push(Command::Load(address));
            }
            Target::Browser(BrowserEvent::BrowserZoom) => {
                browser.commands.push(Command::Zoom(ZoomStep::Reset))
            }
            Target::Browser(BrowserEvent::BrowserPick) => {
                browser.end_area();
                let tab = &mut **browser;
                tab.picking = !tab.picking;
                tab.commands.push(Command::Pick(tab.picking));
            }
            Target::Browser(BrowserEvent::BrowserTools) => {
                browser.tools = !browser.tools;
                let open = browser.tools;
                browser.commands.push(if open {
                    Command::OpenTools
                } else {
                    Command::CloseTools
                });
            }
            _ => {}
        }
    }

    /// Takes what `held` keeps in each tab of the browser, each with the number of the tab it is for, leaving the tabs with nothing; nothing when the browser is closed.
    fn take_from_tabs<T>(
        &mut self,
        held: impl Fn(&mut BrowserTab) -> &mut Vec<T>,
    ) -> Vec<(u32, T)> {
        let Some(browser) = self.browser.as_mut() else {
            return Vec::new();
        };
        browser
            .tabs
            .iter_mut()
            .flat_map(|tab| {
                let id = tab.id;
                std::mem::take(held(tab))
                    .into_iter()
                    .map(move |item| (id, item))
            })
            .collect()
    }

    /// What the engine has not yet been told to do, each with the number of the tab it is for, which it is now.
    pub fn take_browser_commands(&mut self) -> Vec<(u32, Command)> {
        self.take_from_tabs(|tab| &mut tab.commands)
    }

    /// What the reader answered to the questions of the pages and to certificates that are not trusted, each with the number of the tab it is for, which the engine has not yet been told.
    pub fn take_browser_answers(&mut self) -> Vec<(u32, Answer)> {
        self.take_from_tabs(|tab| &mut tab.safety.answers)
    }

    /// Clears the notice of every tab that has one and loads its address again, as after the browser was installed.
    pub fn browser_retry(&mut self) {
        let host = self.current_project_host();
        if let Some(browser) = self.browser.as_mut() {
            for tab in &mut browser.tabs {
                if tab.notice.take().is_none() {
                    continue;
                }
                if tab.sealed.is_some() {
                    tab.commands.push(Command::LoadSealed);
                } else {
                    tab.host.clone_from(&host);
                    let address = address_of(&tab.address.text());
                    tab.commands.push(Command::Load(address));
                }
            }
        }
    }

    /// The tab numbered `tab` of the browser, when the browser is open and has it.
    pub(crate) fn browser_tab(&mut self, tab: u32) -> Option<&mut BrowserTab> {
        self.browser.as_mut()?.tab_mut(tab)
    }

    /// The state after the mode of picking an element ended in tab `tab`: the pick becomes a chip on the prompt of the chat and the pane being looked at stays as it is; no element means the mode ended without a pick.
    pub fn browser_picked(mut self, tab: u32, element: Option<crate::elements::Element>) -> Self {
        if let Some(tab) = self.browser_tab(tab) {
            tab.picking = false;
        }
        match element {
            Some(element) => self.with_element(element),
            None => self,
        }
    }

    /// Takes in the zoom the engine reported for the page of tab `tab`, as a factor.
    pub fn browser_zoomed(&mut self, tab: u32, factor: f32) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.zoom = factor;
        }
    }

    /// The state after the helper reported, for tab `tab`, what the reader asked to add to the prompt: it becomes a chip, and the mode of dragging a rectangle, if it was on, is over; no capture means the mode ended without an area.
    pub fn browser_captured(mut self, tab: u32, capture: Option<crate::captures::Capture>) -> Self {
        if let Some(tab) = self.browser_tab(tab) {
            tab.picking_area = false;
        }
        match capture {
            Some(capture) => self.with_capture(capture),
            None => self,
        }
    }

    /// Takes away the notice of tab `tab`, as its page is about to be shown.
    pub fn browser_notice_cleared(&mut self, tab: u32) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.notice = None;
        }
    }

    /// Says why the page of tab `tab` cannot be shown.
    pub fn browser_tab_notice(&mut self, tab: u32, notice: &str) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.notice = Some(notice.to_owned());
        }
    }

    /// Says why no page can be shown, in every tab.
    pub fn browser_notice(&mut self, notice: &str) {
        if let Some(browser) = self.browser.as_mut() {
            for tab in &mut browser.tabs {
                tab.notice = Some(notice.to_owned());
            }
        }
    }

    /// Records the newest picture of the developer tools.
    pub fn browser_tools_painted(&mut self, tools_page: rustle::ImageData) {
        if let Some(browser) = self.browser.as_mut() {
            browser.tools_page = Some(tools_page);
        }
    }

    /// Records what the engine reports of the page of tab `tab`: its picture and title, how the pointer looks, and, unless the address is being typed in the tab shown, its address.
    pub fn browser_tab_painted(
        &mut self,
        tab: u32,
        page: Option<rustle::ImageData>,
        title: &str,
        address: &str,
        pointer: Pointer,
        typing: bool,
    ) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        let shown = browser.active_id() == tab;
        let Some(tab) = browser.tab_mut(tab) else {
            return;
        };
        if page.is_some() {
            tab.page = page;
        }
        title.clone_into(&mut tab.title);
        tab.pointer = pointer;
        if !(typing && shown) && !address.is_empty() && tab.address.text() != address {
            tab.address = TextFieldState::new(address);
        }
    }
}

/// The look of a button of the toolbar that turns a mode on and off: filled while `on`, bare otherwise.
fn toggled(on: bool) -> Variant {
    if on {
        Variant::Secondary
    } else {
        Variant::Ghost
    }
}

/// Emits the bar above the page of `browser`: back, forward, reload, the address, the zoom when it is not 100%, picking, capturing, the lock and the developer tools.
fn toolbar(browser: &Browser) {
    let zoomed = (browser.zoom - 1.0).abs() > 0.005;
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(TOOLBAR)
            .border_bottom(1.0, theme::color(Role::Border))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(2), 0.0),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            for (icon, id) in [
                (phosphor::ARROW_LEFT, BrowserEvent::BrowserBack),
                (phosphor::ARROW_RIGHT, BrowserEvent::BrowserForward),
                (phosphor::ARROWS_CLOCKWISE, BrowserEvent::BrowserReload),
            ] {
                Button::icon(icon, Target::Browser(id)).build();
            }
            rustle::column_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    Input::new(
                        &browser.address,
                        Target::Browser(BrowserEvent::BrowserAddress),
                    )
                    .size(Size::Sm)
                    .placeholder("Search or enter an address")
                    .modifier(Modifier::new().fill_max_width())
                    .submits(Target::Browser(BrowserEvent::BrowserGo))
                    .build();
                },
            );
            if zoomed {
                button::action(
                    format!("{}%", (browser.zoom * 100.0).round()),
                    Target::Browser(BrowserEvent::BrowserZoom),
                    Variant::Ghost,
                );
            }
            Button::icon(
                phosphor::SELECTION,
                Target::Browser(BrowserEvent::BrowserPick),
            )
            .variant(toggled(browser.picking))
            .build();
            let operations = crate::events::operations();
            Button::icon(
                phosphor::CAMERA,
                Target::Browser(BrowserEvent::BrowserCapture),
            )
            .on_click(move || operations.open_browser_capture_menu())
            .variant(toggled(browser.picking_area))
            .build();
            Button::icon(
                if browser.locked {
                    phosphor::LOCK_SIMPLE
                } else {
                    phosphor::LOCK_SIMPLE_OPEN
                },
                Target::Browser(BrowserEvent::BrowserLock),
            )
            .variant(toggled(browser.locked))
            .disabled(browser.agent)
            .tooltip(if browser.agent {
                "Opened by an agent: agents may always work in this tab"
            } else if browser.locked {
                "Locked: agents cannot change this tab. Press to unlock it."
            } else {
                "Lock this tab so agents cannot change it"
            })
            .build();
            let tools = browser.tools;
            Button::icon(
                phosphor::SIDEBAR_SIMPLE,
                Target::Browser(BrowserEvent::BrowserTools),
            )
            .mark(move || panel_right(tools, 4.0, Role::Muted))
            .build();
        },
    );
}

/// Emits the window `id` of `browser`, sized by `size`: the certificate the page is stopped at, laid out for the room it is given as Compose's `BoxWithConstraints` lays it out, the newest `shown` picture, or what the tab says instead of a page. The pointer over it is handed on to the page unless the question about clearing covers it or the pointer is over what is drawn over the page, as `covered` says.
#[track_caller]
fn surface(
    browser: &Browser,
    id: Target,
    shown: &Option<rustle::ImageData>,
    size: Modifier,
    covered: &Rc<Cell<bool>>,
) {
    if id == Target::Browser(BrowserEvent::BrowserBody)
        && let Some(certificate) = browser.safety.certificate.as_ref()
    {
        let certificate = certificate.clone();
        rustle::with_constraints(size, move |room| {
            certificate_page(&certificate, room.max_width, room.max_height);
        });
        return;
    }
    let page = PagePointer::new(id, !browser.clearing, Rc::clone(covered));
    let on_pointer = page.on_pointer();
    let body = size
        .clip()
        .cursor(cursor_of(browser.pointer))
        .chain(page.modifier())
        .pressable_pointing(
            id,
            send(Event::pressing(id)),
            {
                let operations = crate::events::operations();
                move |at| {
                    if id == Target::Browser(BrowserEvent::BrowserBody) {
                        operations.open_browser_menu(at);
                    } else {
                        operations.open_message_menu(None, at);
                    }
                }
            },
            move |event| on_pointer(event),
        )
        .chain(crate::bounds::placed(id, Modifier::new()))
        .chain(crate::bounds::sized(id, Modifier::new()))
        .chain(crate::focus::keyboard_focus(id));
    match shown {
        Some(shown)
            if browser.notice.is_none() || id != Target::Browser(BrowserEvent::BrowserBody) =>
        {
            rustle::image(body, shown, rustle::ContentScale::Crop)
        }
        _ => rustle::column_arranged(body, Arrangement::Start, Alignment::Start, || {
            line(
                browser.notice.as_deref().unwrap_or("Loading"),
                12,
                1.4,
                Role::Muted,
            );
        }),
    }
}

/// Emits the line between the page and the developer tools, as tall as the row it is in, with the handle that drags it.
fn divider() {
    rustle::stack_with(
        Modifier::new()
            .width(1.0)
            .fill_max_height()
            .background(theme::color(Role::Border)),
        || {
            Sash::column(Target::Browser(BrowserEvent::BrowserSash))
                .bare()
                .build()
        },
    );
}

/// The size of the developer tools beside the page: `width` logical pixels wide, as they were dragged, kept whole and within what leaves them and the page [`TOOLS_LEAST`] each of the room the row has left for them after the line between them, and as tall as the row.
fn tools_size(width: f32) -> Modifier {
    Modifier::new()
        .fill_max_height()
        .layout(move |content, room| {
            let wide = width
                .clamp(TOOLS_LEAST, (room.max_width - TOOLS_LEAST).max(TOOLS_LEAST))
                .floor();
            let size = content.measure(rustle::Constraints {
                min_width: wide,
                max_width: wide,
                ..room
            });
            rustle::Placement::new(size, rustle::Offset::default())
        })
}

/// Emits the box of the page of `browser`, taking the room its row leaves after the developer tools: the page, the bar for finding in it over its top right corner and the question laid over its top, which report whether the pointer is over them in `covered`.
#[track_caller]
fn page_box(browser: &Browser, covered: &Rc<Cell<bool>>) {
    rustle::stack_with(Modifier::new().weight(1.0).fill_max_height(), || {
        surface(
            browser,
            Target::Browser(BrowserEvent::BrowserBody),
            &browser.page,
            Modifier::new().fill_max_size(),
            covered,
        );
        if let Some(search) = browser.search.as_ref() {
            let over = Rc::clone(covered);
            find_bar(search, move |hovered| over.set(hovered));
        }
        browser.safety.question_overlay();
    });
}

/// Emits the browser shown in a pane `width` logical pixels wide, filling the pane's height below its strip of tabs: the strip, the toolbar and the bars over the page and the developer tools beside it, which take the height left; nothing when it is closed.
pub fn browser_view(state: &WorkspaceState, width: f32) {
    let Some(browser) = state.browser.as_ref() else {
        return;
    };
    let covered = rustle::remember(|| Rc::new(Cell::new(false))).get();
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .background(theme::color(Role::Background)),
        Arrangement::Start,
        Alignment::Start,
        || {
            tab_strip(browser, width);
            toolbar(browser);
            browser.safety.bar_nodes();
            rustle::row_arranged(
                Modifier::new().fill_max_width().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    page_box(browser, &covered);
                    if browser.tools {
                        divider();
                        surface(
                            browser,
                            Target::Browser(BrowserEvent::BrowserToolsBody),
                            &browser.tools_page,
                            tools_size(browser.tools_width),
                            &covered,
                        );
                    }
                },
            );
        },
    );
}

#[cfg(test)]
mod tests;
