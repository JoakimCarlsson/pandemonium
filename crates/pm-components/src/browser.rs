//! The browser pane: a page of the web beside the agent's work, with its address bar and, beside it, the developer tools of the page.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, bg, border_b, border_color, cursor_text, flex, flex_1, flex_col, flex_none, gap, h_px,
    items_center, min_w_px, overflow_hidden, px, relative, w_full, w_px,
};
use pm_text::Field;
use pm_ui::{Node, Picture, node};

use crate::browser_address::address_of;
use crate::browser_find::{PageSearch, find_bar};
use crate::browser_safety::{Answer, Safety, certificate_page};
use crate::button::Variant;
use crate::parts::line;
use crate::sash::Sash;
use crate::size::Size;
use crate::state::{Tab, WorkspaceState};
use crate::{Button, Input};

/// The tab of the browser in a pane's strip.
pub const TAB_BROWSER: FocusId = FocusId(7_440_000);

/// Closes the browser.
pub const BROWSER_CLOSE: FocusId = FocusId(7_440_001);

/// The window of the page, which takes the pointer and the keyboard while it has focus.
pub const BROWSER_BODY: FocusId = FocusId(7_440_002);

/// The line the address of the page is typed in.
pub const BROWSER_ADDRESS: FocusId = FocusId(7_440_003);

/// Goes back one page in the history.
pub const BROWSER_BACK: FocusId = FocusId(7_440_004);

/// Goes forward one page in the history.
pub const BROWSER_FORWARD: FocusId = FocusId(7_440_005);

/// Loads the page again.
pub const BROWSER_RELOAD: FocusId = FocusId(7_440_006);

/// Shows or hides the developer tools beside the page.
pub const BROWSER_TOOLS: FocusId = FocusId(7_440_007);

/// The window of the developer tools, which takes the pointer and the keyboard while it has focus.
pub const BROWSER_TOOLS_BODY: FocusId = FocusId(7_440_008);

/// Loads the address that was typed.
pub const BROWSER_GO: FocusId = FocusId(7_440_009);

/// Turns on or off picking an element of the page to send to the agent.
pub const BROWSER_PICK: FocusId = FocusId(7_440_010);

/// The handle between the page and its developer tools, dragged to change how wide the tools are.
pub const BROWSER_SASH: FocusId = FocusId(7_440_011);

/// The line the text to find in the page is typed in.
pub const BROWSER_FIND_FIELD: FocusId = FocusId(7_440_012);

/// Goes to the next match of the text found in the page.
pub const BROWSER_FIND_NEXT: FocusId = FocusId(7_440_013);

/// Goes to the previous match of the text found in the page.
pub const BROWSER_FIND_PREVIOUS: FocusId = FocusId(7_440_014);

/// Closes the bar for finding in the page.
pub const BROWSER_FIND_CLOSE: FocusId = FocusId(7_440_015);

/// The bar for finding in the page.
pub const BROWSER_FIND_BAR: FocusId = FocusId(7_440_016);

/// The zoom of the page, shown while it is not 100%; pressing it puts the zoom back.
pub const BROWSER_ZOOM: FocusId = FocusId(7_440_017);

/// The dimmed window behind the question about clearing cookies and site data.
pub const BROWSER_CLEAR_OUTSIDE: FocusId = FocusId(7_440_018);

/// The panel of the question about clearing cookies and site data.
pub const BROWSER_CLEAR_INSIDE: FocusId = FocusId(7_440_019);

/// Clears the cookies and site data.
pub const BROWSER_CLEAR_CONFIRM: FocusId = FocusId(7_440_020);

/// Leaves the cookies and site data as they are.
pub const BROWSER_CLEAR_CANCEL: FocusId = FocusId(7_440_021);

/// Opens the menu of what can be added to the prompt from the page: pictures of it and its console.
pub const BROWSER_CAPTURE: FocusId = FocusId(7_440_900);

/// One past the last thing of the browser.
const BROWSER_END: u64 = 7_441_000;

/// Where a page opens when the browser is opened with nothing typed.
pub const HOME: &str = "http://localhost:5173";

/// How tall the bar above the page is, in logical pixels.
pub const TOOLBAR: f32 = 44.0;

/// How wide the developer tools are until they are dragged, in logical pixels.
const TOOLS_WIDTH: f32 = 500.0;

/// How narrow the developer tools, and the page beside them, can be made, in logical pixels.
pub const TOOLS_LEAST: f32 = 200.0;

/// The surface number the page's pictures are drawn on.
pub const PAGE_SURFACE: u32 = 1;

/// The surface number the developer tools' pictures are drawn on.
pub const TOOLS_SURFACE: u32 = 2;

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
    /// Close the page.
    Close,
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

/// The browser: what its page last painted and what the window has asked of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Browser {
    /// The line the address is typed in.
    pub address: Field,
    /// The title of the page.
    pub title: String,
    /// The newest picture of the page.
    pub page: Option<Picture>,
    /// The newest picture of the developer tools.
    pub tools_page: Option<Picture>,
    /// Whether the developer tools are open beside the page.
    pub tools: bool,
    /// How wide the developer tools are when open, in logical pixels.
    pub tools_width: f32,
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
    /// Whether the question about clearing cookies and site data is open.
    pub clearing: bool,
    /// What the page asked of the reader, a site whose certificate is not trusted, and the last download.
    pub safety: Safety,
    /// What the engine has not yet been told to do.
    pub commands: Vec<Command>,
}

impl Browser {
    /// A browser about to load `address`.
    fn new(address: &str, host: Option<String>) -> Self {
        Self {
            address: Field::new(address),
            title: String::new(),
            page: None,
            tools_page: None,
            tools: false,
            tools_width: TOOLS_WIDTH,
            pointer: Pointer::Arrow,
            picking: false,
            picking_area: false,
            host,
            notice: None,
            search: None,
            zoom: 1.0,
            clearing: false,
            safety: Safety::default(),
            commands: vec![Command::Load(address.to_owned())],
        }
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

/// What a browser showing `current` does when a page is found at `address`.
#[derive(Debug, PartialEq, Eq)]
enum Move {
    /// It is on another page, which the reader or an agent is using, and stays.
    Stay,
    /// It shows nothing or an error, and loads the page.
    Load,
    /// It shows a page of the same server, which started again, and loads that page again where it is.
    Reload,
}

/// What a browser showing `current` does when a page is found at `address`.
fn served_move(current: &str, address: &str) -> Move {
    let current = current.trim();
    if current.is_empty() || current == "about:blank" || current.starts_with("chrome-error:") {
        return Move::Load;
    }
    let origin = address.trim_end_matches('/');
    let same = current
        .strip_prefix(origin)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?', '#']));
    if same { Move::Reload } else { Move::Stay }
}

/// Whether `id` is something of the browser.
pub fn is_browser_part(id: FocusId) -> bool {
    (TAB_BROWSER.0..BROWSER_END).contains(&id.0)
}

impl WorkspaceState {
    /// The machine of the project the reader is in when it is not this one.
    fn current_project_host(&self) -> Option<String> {
        self.projects.get(self.current_project())?.host()
    }

    /// Opens the browser on `address`, or shows it when it is open, and shows it.
    pub fn open_browser(&mut self, address: &str) {
        let host = self.current_project_host();
        if self.browser.is_none() {
            self.browser = Some(Browser::new(address, host));
        }
        self.tab = Tab::Browser;
    }

    /// Shows the page a program started in a shell serves at `address` on the machine `host`: the browser opens on it, or when it is open and is on nothing, on an error or on a page of the same server, it goes to it.
    /// A browser on another page is left on it, since the reader or an agent is using it.
    pub fn open_served_page(&mut self, address: &str, host: Option<String>) {
        match self.browser.as_mut() {
            Some(browser) => match served_move(browser.address.text(), address) {
                Move::Stay => return,
                Move::Load => {
                    browser.host = host;
                    browser.address = Field::new(address);
                    browser.commands.push(Command::Load(address.to_owned()));
                }
                Move::Reload => {
                    browser.host = host;
                    browser.commands.push(Command::Reload);
                }
            },
            None => self.browser = Some(Browser::new(address, host)),
        }
        self.tab = Tab::Browser;
    }

    /// Opens the browser as it was left: on `address`, on the machine `host`, with the developer tools open when `tools`, `tools_width` wide, shown when `shown`.
    pub fn restore_browser(
        &mut self,
        address: &str,
        host: Option<String>,
        tools: bool,
        tools_width: f32,
        shown: bool,
    ) {
        let mut browser = Browser::new(address, host);
        if tools_width > 0.0 {
            browser.tools_width = tools_width;
        }
        if tools {
            browser.tools = true;
            browser.commands.push(Command::OpenTools);
        }
        self.browser = Some(browser);
        if shown {
            self.tab = Tab::Browser;
        }
    }

    /// How the browser is to be put back after a restart: its address, its machine, whether its developer tools are open and how wide they are.
    pub fn browser_saved(&self) -> Option<(String, Option<String>, bool, f32)> {
        let browser = self.browser.as_ref()?;
        Some((
            browser.address.text().to_owned(),
            browser.host.clone(),
            browser.tools,
            browser.tools_width,
        ))
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
        if let Some(browser) = self.browser.as_mut() {
            browser.commands.push(Command::Close);
        }
        self.browser = None;
        if self.tab == Tab::Browser {
            self.tab = Tab::Chat;
        }
        self
    }

    /// Whether the browser is open.
    pub fn browser_open(&self) -> bool {
        self.browser.is_some()
    }

    /// Carries out what pressing `id` of the browser does.
    pub(crate) fn browser_pressed(mut self, id: FocusId) -> Self {
        match id {
            BROWSER_CLOSE => return self.browser_closed(),
            BROWSER_FIND_NEXT => {
                self.browser_find_stepped(false);
                return self;
            }
            BROWSER_FIND_PREVIOUS => {
                self.browser_find_stepped(true);
                return self;
            }
            BROWSER_FIND_CLOSE => {
                self.browser_find_closed();
                return self;
            }
            BROWSER_CLEAR_CONFIRM => {
                self.browser_clear_confirmed();
                return self;
            }
            BROWSER_CLEAR_CANCEL | BROWSER_CLEAR_OUTSIDE => {
                self.browser_clear_declined();
                return self;
            }
            _ => {}
        }
        let host = self.current_project_host();
        let Some(browser) = self.browser.as_mut() else {
            return self;
        };
        if browser.safety.press(id) {
            return self;
        }
        match id {
            TAB_BROWSER => self.tab = Tab::Browser,
            BROWSER_BACK => browser.commands.push(Command::Back),
            BROWSER_FORWARD => browser.commands.push(Command::Forward),
            BROWSER_RELOAD => browser.commands.push(Command::Reload),
            BROWSER_GO => {
                let address = address_of(browser.address.text());
                browser.host = host;
                browser.commands.push(Command::Load(address));
            }
            BROWSER_ZOOM => browser.commands.push(Command::Zoom(ZoomStep::Reset)),
            BROWSER_PICK => {
                browser.end_area();
                browser.picking = !browser.picking;
                browser.commands.push(Command::Pick(browser.picking));
            }
            BROWSER_TOOLS => {
                browser.tools = !browser.tools;
                browser.commands.push(if browser.tools {
                    Command::OpenTools
                } else {
                    Command::CloseTools
                });
            }
            _ => {}
        }
        self
    }

    /// What the engine has not yet been told to do, which it is now.
    pub fn take_browser_commands(&mut self) -> Vec<Command> {
        self.browser
            .as_mut()
            .map(|browser| std::mem::take(&mut browser.commands))
            .unwrap_or_default()
    }

    /// What the reader answered to the page's questions and to a certificate that is not trusted, which the engine has not yet been told.
    pub fn take_browser_answers(&mut self) -> Vec<Answer> {
        self.browser
            .as_mut()
            .map(|browser| std::mem::take(&mut browser.safety.answers))
            .unwrap_or_default()
    }

    /// Clears the notice and loads the address in the bar again, as after the browser was installed.
    pub fn browser_retry(&mut self) {
        let host = self.current_project_host();
        if let Some(browser) = self.browser.as_mut() {
            browser.notice = None;
            browser.host = host;
            let address = address_of(browser.address.text());
            browser.commands.push(Command::Load(address));
        }
    }

    /// The state after the mode of picking an element ended: the pick becomes a chip on the prompt of the chat and the pane being looked at stays as it is; no element means the mode ended without a pick.
    pub fn browser_picked(mut self, element: Option<crate::elements::Element>) -> Self {
        if let Some(browser) = self.browser.as_mut() {
            browser.picking = false;
        }
        match element {
            Some(element) => self.with_element(element),
            None => self,
        }
    }

    /// Takes in the zoom the engine reported for the page, as a factor.
    pub fn browser_zoomed(&mut self, factor: f32) {
        if let Some(browser) = self.browser.as_mut() {
            browser.zoom = factor;
        }
    }

    /// The state after the helper reported what the reader asked to add to the prompt: it becomes a chip, and the mode of dragging a rectangle, if it was on, is over; no capture means the mode ended without an area.
    pub fn browser_captured(mut self, capture: Option<crate::captures::Capture>) -> Self {
        if let Some(browser) = self.browser.as_mut() {
            browser.picking_area = false;
        }
        match capture {
            Some(capture) => self.with_capture(capture),
            None => self,
        }
    }

    /// Takes away the notice, as the page is about to be shown.
    pub fn browser_notice_cleared(&mut self) {
        if let Some(browser) = self.browser.as_mut() {
            browser.notice = None;
        }
    }

    /// Says why the page cannot be shown.
    pub fn browser_notice(&mut self, notice: &str) {
        if let Some(browser) = self.browser.as_mut() {
            browser.notice = Some(notice.to_owned());
        }
    }

    /// Records what the engine reports: the pictures and title of the page, and, unless the address is being typed, its address.
    pub fn browser_painted(
        &mut self,
        page: Option<Picture>,
        tools_page: Option<Picture>,
        title: &str,
        address: &str,
        pointer: Pointer,
        typing: bool,
    ) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        if page.is_some() {
            browser.page = page;
        }
        if tools_page.is_some() {
            browser.tools_page = tools_page;
        }
        browser.title = title.to_owned();
        browser.pointer = pointer;
        if !typing && !address.is_empty() && browser.address.text() != address {
            browser.address = Field::new(address);
        }
    }
}

/// The browser shown in a pane `width` by `height` logical pixels.
pub fn browser_view(state: &WorkspaceState, width: f32, height: f32) -> Node {
    let Some(browser) = state.browser.as_ref() else {
        return node();
    };
    let zoomed = (browser.zoom - 1.0).abs() > 0.005;
    let toolbar = node()
        .styles([
            flex(),
            items_center(),
            gap(1),
            px(2),
            h_px(TOOLBAR),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(Button::icon(IconName::ArrowLeft, BROWSER_BACK).build())
        .child(Button::icon(IconName::ArrowRight, BROWSER_FORWARD).build())
        .child(Button::icon(IconName::Refresh, BROWSER_RELOAD).build())
        .child(
            node().styles([flex_1(), min_w_px(0)]).child(
                Input::new(&browser.address, BROWSER_ADDRESS)
                    .size(Size::Sm)
                    .placeholder("Search or enter an address")
                    .build(),
            ),
        )
        .children(zoomed.then(|| {
            Button::new(format!("{}%", (browser.zoom * 100.0).round()), BROWSER_ZOOM)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build()
        }))
        .child(
            Button::icon(IconName::Pick, BROWSER_PICK)
                .variant(if browser.picking {
                    Variant::Secondary
                } else {
                    Variant::Ghost
                })
                .build(),
        )
        .child(
            Button::icon(IconName::Camera, BROWSER_CAPTURE)
                .variant(if browser.picking_area {
                    Variant::Secondary
                } else {
                    Variant::Ghost
                })
                .build(),
        )
        .child(
            Button::icon(
                if browser.tools {
                    IconName::PanelRightFilled
                } else {
                    IconName::PanelRight
                },
                BROWSER_TOOLS,
            )
            .build(),
        );
    let room = (height - TOOLBAR - browser.safety.bars()).max(0.0);
    let surface = |id: FocusId, picture: &Option<Picture>, wide: f32, tall: f32| {
        if id == BROWSER_BODY
            && let Some(certificate) = browser.safety.certificate.as_ref()
        {
            return certificate_page(certificate, wide, tall);
        }
        let body = node()
            .styles([
                flex_none(),
                w_px(wide),
                h_px(tall),
                overflow_hidden(),
                cursor_text(),
            ])
            .focusable(id);
        match picture {
            Some(picture) if browser.notice.is_none() || id != BROWSER_BODY => {
                body.picture(picture.clone())
            }
            _ => body.child(line(
                browser.notice.as_deref().unwrap_or("Loading"),
                12,
                1.4,
                Role::Muted,
            )),
        }
    };
    let divider = |wide: f32, tall: f32| {
        node().styles([w_px(wide), h_px(tall), bg(Role::Border), flex_none()])
    };
    let mut beside = 0.0;
    let mut page_width = width;
    let mut row = node().styles([flex(), w_full(), h_px(room), relative()]);
    if browser.tools {
        let tools = browser
            .tools_width
            .clamp(TOOLS_LEAST, (width - TOOLS_LEAST - 1.0).max(TOOLS_LEAST))
            .floor();
        let page = (width - tools - 1.0).max(0.0);
        beside = tools + 1.0;
        page_width = page;
        row = row
            .child(surface(BROWSER_BODY, &browser.page, page, room))
            .child(
                divider(1.0, room)
                    .styles([relative()])
                    .child(Sash::column(BROWSER_SASH).bare().build()),
            )
            .child(surface(
                BROWSER_TOOLS_BODY,
                &browser.tools_page,
                tools,
                room,
            ));
    } else {
        row = row.child(surface(BROWSER_BODY, &browser.page, width, room));
    }
    row = row.children(
        browser
            .search
            .as_ref()
            .map(|search| find_bar(search, page_width, beside)),
    );
    let mut pane = node()
        .styles([
            flex(),
            flex_col(),
            w_full(),
            h_px(height),
            bg(Role::Background),
        ])
        .child(toolbar);
    for bar in browser.safety.bar_nodes() {
        pane = pane.child(bar);
    }
    pane.child(row)
}

#[cfg(test)]
mod tests;
