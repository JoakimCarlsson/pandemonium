//! The menu that opens over a tab of the browser's strip: opening a new tab or a private one, loading the tab again, pinning or unpinning it, and closing it or the other tabs.

use crate::Menu;
use crate::areas::MenuEvent;
use crate::browser::Command;
use crate::state::{Tab, WorkspaceState};
use crate::target::Target;

/// What a row of the menu over a tab does.
#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
pub enum BrowserTabCommand {
    /// Opens a new tab and shows it.
    NewTab,
    /// Opens a new private tab and shows it.
    NewPrivateTab,
    /// Loads the page of the tab again.
    Reload,
    /// Pins the tab, or unpins it.
    TogglePin,
    /// Closes the tab.
    Close,
    /// Closes every other tab that is not pinned.
    CloseOthers,
}

impl BrowserTabCommand {
    /// The name of the command's row.
    pub fn id(self) -> Target {
        Target::Menu(MenuEvent::BrowserTabMenu(self))
    }

    /// The command whose row `id` names.
    pub fn of(id: Target) -> Option<Self> {
        match id {
            Target::Menu(MenuEvent::BrowserTabMenu(command)) => Some(command),
            _ => None,
        }
    }
}

/// The menu open over a tab of the browser's strip, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserTabMenu {
    /// The number of the tab it is over.
    pub tab: u32,
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
}

impl WorkspaceState {
    /// The state with the menu over the tab at `index` of the strip open at `at`, and the other menus closed.
    pub fn with_browser_tab_menu(self, index: usize, at: (f32, f32)) -> Self {
        let Some(tab) = self
            .browser
            .as_ref()
            .and_then(|browser| browser.tabs.get(index))
            .map(|tab| tab.id)
        else {
            return self;
        };
        let mut state = self.without_menu();
        state.browser_tab_menu = Some(BrowserTabMenu { tab, at });
        state
    }

    /// Carries out `command` on the tab the menu is open over, and closes the menu.
    pub fn browser_tab_command(mut self, command: BrowserTabCommand) -> Self {
        let Some(open) = self.browser_tab_menu.take() else {
            return self;
        };
        match command {
            BrowserTabCommand::NewTab => self.browser_new_tab(),
            BrowserTabCommand::NewPrivateTab => self.browser_new_private_tab(),
            BrowserTabCommand::Reload => {
                if let Some(tab) = self.browser_tab(open.tab) {
                    tab.commands.push(Command::Reload);
                }
            }
            BrowserTabCommand::TogglePin => {
                if let Some(browser) = self.browser.as_mut()
                    && let Some(index) = browser.tab_index(open.tab)
                {
                    browser.toggle_pin(index);
                }
            }
            BrowserTabCommand::Close => {
                self.browser_tab_closed(open.tab);
            }
            BrowserTabCommand::CloseOthers => {
                let others: Vec<u32> = self
                    .browser
                    .iter()
                    .flat_map(|browser| browser.tabs.iter())
                    .filter(|tab| tab.id != open.tab && !tab.pinned)
                    .map(|tab| tab.id)
                    .collect();
                for id in others {
                    self.browser_tab_closed(id);
                }
            }
        }
        if self.browser.is_some() {
            self.tab = Tab::Browser;
        }
        self
    }
}

/// Emits the menu over a tab of the browser's strip as `open` says, its rows fitting the tab of `state` it is over; nothing when that tab is gone.
#[track_caller]
pub fn browser_tab_menu(state: &WorkspaceState, open: BrowserTabMenu) {
    let Some(browser) = state.browser.as_ref() else {
        return;
    };
    let Some(tab) = browser.tab(open.tab) else {
        return;
    };
    let pin = if tab.pinned { "Unpin Tab" } else { "Pin Tab" };
    let others = browser
        .tabs
        .iter()
        .any(|other| other.id != open.tab && !other.pinned);
    let operations = crate::events::operations();
    use BrowserTabCommand as Do;
    let rows = [
        ("New Tab", Some("Ctrl+T"), Do::NewTab, true, false),
        (
            "New Private Tab",
            Some("Ctrl+Shift+N"),
            Do::NewPrivateTab,
            true,
            true,
        ),
        (
            "Reload",
            Some("Ctrl+R"),
            Do::Reload,
            tab.sealed.is_none(),
            false,
        ),
        (pin, None, Do::TogglePin, true, true),
        ("Close Tab", Some("Ctrl+W"), Do::Close, true, false),
        ("Close Other Tabs", None, Do::CloseOthers, others, false),
    ];
    Menu::new(open.at.0, open.at.1)
        .offset(-8.0, 12.0)
        .width(62.0)
        .row_height(6.5)
        .on_pick(move |row| {
            if let Some(command) = BrowserTabCommand::of(row) {
                operations.update(Box::new(move |state| state.browser_tab_command(command)));
            }
        })
        .rows(rows.map(|(label, keys, command, on, rule)| {
            (label, keys.map(str::to_owned), command.id(), on, rule)
        }))
        .build();
}
