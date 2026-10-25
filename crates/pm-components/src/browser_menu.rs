//! The menu that opens over a page of the browser: going back and forward, loading it again, cutting, copying and pasting, marking everything, and showing the developer tools.

use pm_input::FocusId;
use pm_ui::Node;

use crate::Menu;
use crate::browser::Command;
use crate::state::WorkspaceState;

/// The first of the rows of the menu over a page; each command's row follows it by the command's place.
const BROWSER_MENU_BASE: u64 = 7_422_000;

/// How tall the menu over a page is, in logical pixels, to keep it inside the window.
pub const BROWSER_MENU_HEIGHT: f32 = 8.0 * 26.0 + 2.0 * 9.0 + 10.0;

/// What a row of the menu over a page does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserCommand {
    /// Goes back one page in the history.
    Back,
    /// Goes forward one page in the history.
    Forward,
    /// Loads the page again.
    Reload,
    /// Cuts what is marked in the page.
    Cut,
    /// Copies what is marked in the page.
    Copy,
    /// Pastes the clipboard into the page.
    Paste,
    /// Marks everything in the page.
    SelectAll,
    /// Shows the developer tools beside the page.
    Inspect,
}

impl BrowserCommand {
    /// Every command, in the order of their rows.
    const ALL: [Self; 8] = [
        Self::Back,
        Self::Forward,
        Self::Reload,
        Self::Cut,
        Self::Copy,
        Self::Paste,
        Self::SelectAll,
        Self::Inspect,
    ];

    /// The name of the command's row.
    pub fn id(self) -> FocusId {
        let place = Self::ALL
            .iter()
            .position(|held| *held == self)
            .unwrap_or_default();
        FocusId(BROWSER_MENU_BASE + place as u64)
    }

    /// The command whose row `id` names.
    pub fn of(id: FocusId) -> Option<Self> {
        let place = id.0.checked_sub(BROWSER_MENU_BASE)?;
        Self::ALL.get(usize::try_from(place).ok()?).copied()
    }

    /// What its row says.
    fn label(self) -> &'static str {
        match self {
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::Reload => "Reload",
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
            Self::Inspect => "Inspect",
        }
    }
}

/// The menu open over a page of the browser, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserMenu {
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
}

impl WorkspaceState {
    /// The state with the menu over the page open at `at`, and the other menus closed.
    pub fn with_browser_menu(self, at: (f32, f32)) -> Self {
        if self.browser.is_none() {
            return self;
        }
        let mut state = self.without_menu();
        state.browser_menu = Some(BrowserMenu { at });
        state
    }

    /// Whether `id` is a row of the menu over a page while it is open.
    pub fn is_browser_menu_row(&self, id: FocusId) -> bool {
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
            BrowserCommand::Cut => browser.commands.push(Command::Cut),
            BrowserCommand::Copy => browser.commands.push(Command::Copy),
            BrowserCommand::Paste => browser.commands.push(Command::Paste),
            BrowserCommand::SelectAll => browser.commands.push(Command::SelectAll),
            BrowserCommand::Inspect => {
                if !browser.tools {
                    browser.tools = true;
                    browser.commands.push(Command::OpenTools);
                }
            }
        }
        self
    }
}

/// The menu over a page opened at `open`.
pub fn browser_menu(open: BrowserMenu) -> Node {
    let row =
        |menu: Menu, command: BrowserCommand| menu.item(command.label(), None, command.id(), false);
    let menu = Menu::new(open.at.0, open.at.1).width(48.0).row_height(6.5);
    let menu = row(menu, BrowserCommand::Back);
    let menu = row(menu, BrowserCommand::Forward);
    let menu = row(menu, BrowserCommand::Reload).separator();
    let menu = row(menu, BrowserCommand::Cut);
    let menu = row(menu, BrowserCommand::Copy);
    let menu = row(menu, BrowserCommand::Paste);
    let menu = row(menu, BrowserCommand::SelectAll).separator();
    row(menu, BrowserCommand::Inspect).build()
}
