//! The browser pane: a page of the web beside the agent's work, with its address bar and, beside it, the developer tools of the page.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, bg, border_b, border_color, cursor_text, flex, flex_1, flex_col, flex_none, gap, h_px,
    items_center, min_w_px, overflow_hidden, px, w_full, w_px,
};
use pm_text::Field;
use pm_ui::{Node, Picture, node};

use crate::parts::line;
use crate::size::Size;
use crate::state::{Tab, WorkspaceState};
use crate::{Button, Input};

/// The tab of the browser in a pane's strip.
pub const TAB_BROWSER: FocusId = FocusId(7_420_000);

/// Closes the browser.
pub const BROWSER_CLOSE: FocusId = FocusId(7_420_001);

/// The window of the page, which takes the pointer and the keyboard while it has focus.
pub const BROWSER_BODY: FocusId = FocusId(7_420_002);

/// The line the address of the page is typed in.
pub const BROWSER_ADDRESS: FocusId = FocusId(7_420_003);

/// Goes back one page in the history.
pub const BROWSER_BACK: FocusId = FocusId(7_420_004);

/// Goes forward one page in the history.
pub const BROWSER_FORWARD: FocusId = FocusId(7_420_005);

/// Loads the page again.
pub const BROWSER_RELOAD: FocusId = FocusId(7_420_006);

/// Shows or hides the developer tools beside the page.
pub const BROWSER_TOOLS: FocusId = FocusId(7_420_007);

/// The window of the developer tools, which takes the pointer and the keyboard while it has focus.
pub const BROWSER_TOOLS_BODY: FocusId = FocusId(7_420_008);

/// Loads the address that was typed.
pub const BROWSER_GO: FocusId = FocusId(7_420_009);

/// One past the last thing of the browser.
const BROWSER_END: u64 = 7_421_000;

/// How tall the bar above the page is, in logical pixels.
pub const TOOLBAR: f32 = 44.0;

/// The surface number the page's pictures are drawn on.
pub const PAGE_SURFACE: u32 = 1;

/// The surface number the developer tools' pictures are drawn on.
pub const TOOLS_SURFACE: u32 = 2;

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
    /// Open the developer tools.
    OpenTools,
    /// Close the developer tools.
    CloseTools,
    /// Close the page.
    Close,
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
    /// How the pointer looks over the page.
    pub pointer: Pointer,
    /// What the engine has not yet been told to do.
    pub commands: Vec<Command>,
}

impl Browser {
    /// A browser about to load `address`.
    fn new(address: &str) -> Self {
        Self {
            address: Field::new(address),
            title: String::new(),
            page: None,
            tools_page: None,
            tools: false,
            pointer: Pointer::Arrow,
            commands: vec![Command::Load(address.to_owned())],
        }
    }
}

/// The address `typed` names: a web address as it is, a bare host with a scheme in front, and a local host over plain HTTP.
pub fn address_of(typed: &str) -> String {
    let typed = typed.trim();
    if typed.contains("://") || typed.starts_with("about:") || typed.starts_with("data:") {
        return typed.to_owned();
    }
    let local = typed.starts_with("localhost")
        || typed.starts_with("127.")
        || typed.starts_with("0.0.0.0")
        || typed.starts_with('[');
    if local || typed.starts_with(':') {
        let typed = typed
            .strip_prefix(':')
            .map_or(typed.to_owned(), |port| format!("localhost:{port}"));
        return format!("http://{typed}");
    }
    format!("https://{typed}")
}

/// Whether `id` is something of the browser.
pub fn is_browser_part(id: FocusId) -> bool {
    (TAB_BROWSER.0..BROWSER_END).contains(&id.0)
}

impl WorkspaceState {
    /// Opens the browser on `address`, or shows it when it is open, and shows it.
    pub fn open_browser(&mut self, address: &str) {
        if self.browser.is_none() {
            self.browser = Some(Browser::new(address));
        }
        self.tab = Tab::Browser;
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
        if id == BROWSER_CLOSE {
            return self.browser_closed();
        }
        let Some(browser) = self.browser.as_mut() else {
            return self;
        };
        match id {
            TAB_BROWSER => self.tab = Tab::Browser,
            BROWSER_BACK => browser.commands.push(Command::Back),
            BROWSER_FORWARD => browser.commands.push(Command::Forward),
            BROWSER_RELOAD => browser.commands.push(Command::Reload),
            BROWSER_GO => {
                let address = address_of(browser.address.text());
                browser.commands.push(Command::Load(address));
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
    let room = (height - TOOLBAR).max(0.0);
    let surface = |id: FocusId, picture: &Option<Picture>, wide: f32| {
        let body = node()
            .styles([
                flex_none(),
                w_px(wide),
                h_px(room),
                overflow_hidden(),
                cursor_text(),
            ])
            .focusable(id);
        match picture {
            Some(picture) => body.picture(picture.clone()),
            None => body.child(line("Loading", 12, 1.4, Role::Muted)),
        }
    };
    let mut row = node().styles([flex(), w_full(), h_px(room)]);
    if browser.tools {
        let half = (width / 2.0).floor();
        row = row
            .child(surface(BROWSER_BODY, &browser.page, half))
            .child(node().styles([w_px(1.0), h_px(room), bg(Role::Border), flex_none()]))
            .child(surface(
                BROWSER_TOOLS_BODY,
                &browser.tools_page,
                width - half - 1.0,
            ));
    } else {
        row = row.child(surface(BROWSER_BODY, &browser.page, width));
    }
    node()
        .styles([
            flex(),
            flex_col(),
            w_full(),
            h_px(height),
            bg(Role::Background),
        ])
        .child(toolbar)
        .child(row)
}
