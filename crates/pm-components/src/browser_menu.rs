//! The menu that opens over a page of the browser: going back and forward, loading it again with or without the cache, cutting, copying and pasting, marking everything, finding in the page, zooming, opening it in the system's browser, clearing cookies and site data, showing the developer tools, and adding screenshots of the page or its console to the prompt.

use crate::Menu;
use crate::areas::MenuEvent;
use crate::browser::{Command, ZoomStep};
use crate::state::WorkspaceState;
use crate::target::Target;

/// How tall the menu over a page is, in logical pixels, to keep it inside the window.
pub const BROWSER_MENU_HEIGHT: f32 = 19.0 * 26.0 + 6.0 * 9.0 + 10.0;

/// What a row of the menu over a page does.
#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
pub enum BrowserCommand {
    /// Goes back one page in the history.
    Back,
    /// Goes forward one page in the history.
    Forward,
    /// Loads the page again.
    Reload,
    /// Loads the page again without using the cache.
    HardReload,
    /// Cuts what is marked in the page.
    Cut,
    /// Copies what is marked in the page.
    Copy,
    /// Pastes the clipboard into the page.
    Paste,
    /// Marks everything in the page.
    SelectAll,
    /// Shows the bar for finding text in the page.
    Find,
    /// Zooms the page in.
    ZoomIn,
    /// Zooms the page out.
    ZoomOut,
    /// Puts the zoom of the page back to 100%.
    ZoomReset,
    /// Opens the page in the system's browser.
    OpenExternal,
    /// Asks whether to clear the cookies and site data of the browser.
    ClearData,
    /// Shows the developer tools beside the page.
    Inspect,
    /// Adds a picture of what the page shows to the prompt.
    Screenshot,
    /// Adds a picture of the whole page to the prompt.
    FullPage,
    /// Has the reader drag a rectangle over the page and adds a picture of it to the prompt.
    Area,
    /// Adds the lines of the page's console to the prompt.
    Console,
}

impl BrowserCommand {
    /// The commands that put something from the page into the prompt, which the button of the bar offers on their own.
    const CAPTURES: [Self; 4] = [Self::Screenshot, Self::FullPage, Self::Area, Self::Console];

    /// The name of the command's row.
    pub fn id(self) -> Target {
        Target::Menu(MenuEvent::BrowserMenu(self))
    }

    /// The command whose row `id` names.
    pub fn of(id: Target) -> Option<Self> {
        match id {
            Target::Menu(MenuEvent::BrowserMenu(command)) => Some(command),
            _ => None,
        }
    }

    /// What its row says.
    fn label(self) -> &'static str {
        match self {
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::Reload => "Reload",
            Self::HardReload => "Hard Reload",
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
            Self::Find => "Find in Page",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ZoomReset => "Actual Size",
            Self::OpenExternal => "Open in Default Browser",
            Self::ClearData => "Clear Cookies and Site Data...",
            Self::Inspect => "Inspect",
            Self::Screenshot => "Add Screenshot to Chat",
            Self::FullPage => "Add Full Page Screenshot",
            Self::Area => "Add Area Screenshot to Chat",
            Self::Console => "Add Console Logs to Chat",
        }
    }

    /// The keys that do the same while the page has the keyboard, written beside its row.
    fn keys(self) -> Option<&'static str> {
        match self {
            Self::Reload => Some("Ctrl+R"),
            Self::HardReload => Some("Ctrl+Shift+R"),
            Self::Find => Some("Ctrl+F"),
            Self::ZoomIn => Some("Ctrl+="),
            Self::ZoomOut => Some("Ctrl+-"),
            Self::ZoomReset => Some("Ctrl+0"),
            _ => None,
        }
    }
}

/// The menu open over a page of the browser, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserMenu {
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
    /// Whether it offers only what can be put into the prompt, as the button of the bar opens it.
    pub captures_only: bool,
}

impl WorkspaceState {
    /// The state with the menu over the page open at `at`, and the other menus closed.
    pub fn with_browser_menu(self, at: (f32, f32)) -> Self {
        self.with_browser_menu_of(at, false)
    }

    /// The state with the menu of what can be put into the prompt from the page open at `at`, and the other menus closed.
    pub fn with_browser_capture_menu(self, at: (f32, f32)) -> Self {
        self.with_browser_menu_of(at, true)
    }

    /// The state with the menu over the page open at `at`, offering only what can be put into the prompt when `captures_only`, and the other menus closed.
    fn with_browser_menu_of(self, at: (f32, f32), captures_only: bool) -> Self {
        if self.browser.is_none() {
            return self;
        }
        let mut state = self.without_menu();
        state.browser_menu = Some(BrowserMenu { at, captures_only });
        state
    }

    /// Whether `id` is a row of the menu over a page while it is open.
    pub fn is_browser_menu_row(&self, id: Target) -> bool {
        self.browser_menu.is_some() && BrowserCommand::of(id).is_some()
    }

    /// Carries out `command` on the page and closes the menu over it.
    pub fn browser_command(mut self, command: BrowserCommand) -> Self {
        self.browser_menu = None;
        let Some(browser) = self.browser.as_mut() else {
            return self;
        };
        match command {
            BrowserCommand::Back => browser.commands.push(Command::Back),
            BrowserCommand::Forward => browser.commands.push(Command::Forward),
            BrowserCommand::Reload => browser.commands.push(Command::Reload),
            BrowserCommand::HardReload => browser.commands.push(Command::HardReload),
            BrowserCommand::Cut => browser.commands.push(Command::Cut),
            BrowserCommand::Copy => browser.commands.push(Command::Copy),
            BrowserCommand::Paste => browser.commands.push(Command::Paste),
            BrowserCommand::SelectAll => browser.commands.push(Command::SelectAll),
            BrowserCommand::Find => self.browser_find_opened(),
            BrowserCommand::ZoomIn => browser.commands.push(Command::Zoom(ZoomStep::In)),
            BrowserCommand::ZoomOut => browser.commands.push(Command::Zoom(ZoomStep::Out)),
            BrowserCommand::ZoomReset => browser.commands.push(Command::Zoom(ZoomStep::Reset)),
            BrowserCommand::OpenExternal => browser.commands.push(Command::OpenExternal),
            BrowserCommand::ClearData => self.browser_clear_asked(),
            BrowserCommand::Inspect => {
                if !browser.tools {
                    browser.tools = true;
                    browser.commands.push(Command::OpenTools);
                }
            }
            BrowserCommand::Screenshot => browser.commands.push(Command::Screenshot),
            BrowserCommand::FullPage => browser.commands.push(Command::FullPage),
            BrowserCommand::Console => browser.commands.push(Command::Console),
            BrowserCommand::Area => browser.start_area(),
        }
        self
    }
}

/// Emits the menu over a page opened at `open`.
#[track_caller]
pub fn browser_menu(open: BrowserMenu) {
    let row = |menu: Menu, command: BrowserCommand| {
        menu.item(command.label(), command.keys(), command.id(), false)
    };
    let operations = crate::events::operations();
    let menu = Menu::new(open.at.0, open.at.1)
        .width(68.0)
        .row_height(6.5)
        .on_pick(move |row| {
            if let Some(command) = BrowserCommand::of(row) {
                operations.browser_menu_command(command);
            }
        });
    if open.captures_only {
        BrowserCommand::CAPTURES.into_iter().fold(menu, row).build();
        return;
    }
    use BrowserCommand::*;
    let groups: [&[BrowserCommand]; 7] = [
        &[Back, Forward, Reload, HardReload],
        &[Cut, Copy, Paste, SelectAll],
        &[Find],
        &[ZoomIn, ZoomOut, ZoomReset],
        &[OpenExternal, ClearData],
        &[Inspect],
        &BrowserCommand::CAPTURES,
    ];
    groups
        .into_iter()
        .enumerate()
        .fold(menu, |menu, (at, group)| {
            let menu = if at == 0 { menu } else { menu.separator() };
            group.iter().copied().fold(menu, row)
        })
        .build();
}
