//! The menu that opens over a page of the browser: going back and forward, loading it again with or without the cache, cutting, copying and pasting, marking everything, finding in the page, saving it, zooming, opening it in the system's browser, clearing cookies and site data, showing the developer tools, and adding screenshots of the page or its console to the prompt.

use crate::Menu;
use crate::areas::MenuEvent;
use crate::browser::{Command, ZoomStep};
use crate::browser_chrome::{ChromeAction, PageMenu};
use crate::state::WorkspaceState;
use crate::target::Target;

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
    /// Opens the link under the pointer in a new tab.
    OpenLinkInNewTab,
    /// Copies the address of the link under the pointer.
    CopyLink,
    /// Downloads what the link under the pointer leads to.
    SaveLink,
    /// Opens the picture under the pointer in a new tab.
    OpenImageInNewTab,
    /// Copies the picture under the pointer.
    CopyImage,
    /// Copies the address of the picture under the pointer.
    CopyImageAddress,
    /// Downloads the picture under the pointer.
    SaveImage,
    /// Prints the page to a PDF in the Downloads folder and opens it.
    Print,
    /// Puts the spelling checker's suggestion at this place in place of the misspelt word.
    Suggestion(u8),
    /// Saves the page, with the pictures and styles it shows, into the Downloads folder.
    SavePage,
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
            Self::OpenLinkInNewTab => "Open Link in New Tab",
            Self::CopyLink => "Copy Link Address",
            Self::SaveLink => "Save Link As...",
            Self::OpenImageInNewTab => "Open Image in New Tab",
            Self::CopyImage => "Copy Image",
            Self::CopyImageAddress => "Copy Image Address",
            Self::SaveImage => "Save Image As...",
            Self::Print => "Print...",
            Self::Suggestion(_) => "Suggestion",
            Self::SavePage => "Save Page As...",
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
            Self::Print => Some("Ctrl+P"),
            Self::SavePage => Some("Ctrl+S"),
            Self::Inspect => Some("F12"),
            Self::Back => Some("Alt+Left"),
            Self::Forward => Some("Alt+Right"),
            _ => None,
        }
    }
}

/// The menu open over a page of the browser, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserMenu {
    /// Where it opened, in logical pixels, or nothing when it hangs from the button of the
    /// bar that opened it.
    pub at: Option<(f32, f32)>,
    /// Whether it offers only what can be put into the prompt, as the button of the bar opens it.
    pub captures_only: bool,
}

impl WorkspaceState {
    /// The state with the menu over the page open at `at`, and the other menus closed.
    pub fn with_browser_menu(self, at: (f32, f32)) -> Self {
        self.with_browser_menu_of(Some(at), false)
    }

    /// The state with the menu of what can be put into the prompt from the page open, hanging from the button of the bar that opens it, and the other menus closed.
    pub fn with_browser_capture_menu(self) -> Self {
        self.with_browser_menu_of(None, true)
    }

    /// The state with the menu over the page open at `at`, or hanging from its button without a point, offering only what can be put into the prompt when `captures_only`, and the other menus closed.
    fn with_browser_menu_of(self, at: Option<(f32, f32)>, captures_only: bool) -> Self {
        if self.browser.is_none() {
            return self;
        }
        let mut state = self.without_menu();
        state.browser_menu = Some(BrowserMenu { at, captures_only });
        state
    }

    /// Carries out `command` on the page and closes the menu over it.
    pub fn browser_command(mut self, command: BrowserCommand) -> Self {
        self.browser_menu = None;
        let context = self
            .browser
            .as_mut()
            .and_then(|browser| browser.chrome.menu.take());
        if let Some(context) = context.as_ref()
            && self.browser_context_command(command, context)
        {
            return self;
        }
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
            BrowserCommand::Print => browser.commands.push(Command::Chrome(ChromeAction::Print)),
            BrowserCommand::SavePage => browser
                .commands
                .push(Command::Chrome(ChromeAction::SavePage)),
            BrowserCommand::OpenLinkInNewTab
            | BrowserCommand::CopyLink
            | BrowserCommand::SaveLink
            | BrowserCommand::OpenImageInNewTab
            | BrowserCommand::CopyImage
            | BrowserCommand::CopyImageAddress
            | BrowserCommand::SaveImage
            | BrowserCommand::Suggestion(_) => {}
        }
        self
    }

    /// Carries out `command` of the menu that opened over what `context` says was under the pointer, and says whether it was one that needs it.
    fn browser_context_command(&mut self, command: BrowserCommand, context: &PageMenu) -> bool {
        let host = self.current_project_host();
        let Some(browser) = self.browser.as_mut() else {
            return false;
        };
        match command {
            BrowserCommand::OpenLinkInNewTab => {
                browser.open_tab(&context.link, host, false, false);
            }
            BrowserCommand::OpenImageInNewTab => {
                browser.open_tab(&context.source, host, false, false);
            }
            BrowserCommand::CopyLink => browser
                .commands
                .push(Command::CopyText(context.link.clone())),
            BrowserCommand::CopyImageAddress => browser
                .commands
                .push(Command::CopyText(context.source.clone())),
            BrowserCommand::SaveLink => browser
                .commands
                .push(Command::Chrome(ChromeAction::Save(context.link.clone()))),
            BrowserCommand::SaveImage => browser
                .commands
                .push(Command::Chrome(ChromeAction::Save(context.source.clone()))),
            BrowserCommand::CopyImage => {
                browser
                    .commands
                    .push(Command::Chrome(ChromeAction::CopyImageAt(
                        context.x, context.y,
                    )))
            }
            BrowserCommand::Suggestion(at) => {
                if let Some(word) = context.suggestions.get(usize::from(at)) {
                    browser
                        .commands
                        .push(Command::Chrome(ChromeAction::ReplaceMisspelling(
                            word.clone(),
                        )));
                }
            }
            BrowserCommand::Inspect => {
                if !browser.tools {
                    browser.tools = true;
                    browser.commands.push(Command::OpenTools);
                }
                browser
                    .commands
                    .push(Command::Chrome(ChromeAction::InspectAt(
                        context.x, context.y,
                    )));
            }
            _ => return false,
        }
        true
    }
}

/// Emits the menu over a page opened at a point, as `open` says, and nothing for one that
/// hangs from its button, which emits it there ([`capture_dropdown`]).
#[track_caller]
pub fn browser_menu(open: BrowserMenu, context: Option<&PageMenu>) {
    if let Some((x, y)) = open.at {
        let menu = Menu::new(x, y).offset(-8.0, 4.0);
        match context {
            Some(context) => page_menu_rows(context, menu),
            None => browser_menu_rows(open, menu),
        }
        .build();
    }
}

/// `menu` holding the rows of the menu over a page for what `context` says was under the pointer: what can be done with a link, a picture, a misspelt word or what is marked, then moving through the history, saving, printing, inspecting and adding the page to the prompt.
fn page_menu_rows(context: &PageMenu, menu: Menu) -> Menu {
    let row = |menu: Menu, command: BrowserCommand, on: bool| {
        menu.item(command.label(), command.keys(), command.id(), !on)
    };
    let operations = crate::events::operations();
    let mut menu = menu.width(68.0).row_height(6.5).on_pick(move |row| {
        if let Some(command) = BrowserCommand::of(row) {
            operations.browser_menu_command(command);
        }
    });
    use BrowserCommand::*;
    let web = |address: &str| address.starts_with("http://") || address.starts_with("https://");
    let mut groups: Vec<Vec<(BrowserCommand, bool)>> = Vec::new();
    if !context.misspelled.is_empty() {
        let suggestions: Vec<_> = context
            .suggestions
            .iter()
            .take(5)
            .enumerate()
            .map(|(at, _)| (Suggestion(at as u8), true))
            .collect();
        if !suggestions.is_empty() {
            groups.push(suggestions);
        }
    }
    if !context.link.is_empty() {
        groups.push(vec![
            (OpenLinkInNewTab, web(&context.link)),
            (CopyLink, true),
            (SaveLink, true),
        ]);
    }
    if context.image && !context.source.is_empty() {
        groups.push(vec![
            (OpenImageInNewTab, web(&context.source)),
            (CopyImage, true),
            (CopyImageAddress, true),
            (SaveImage, true),
        ]);
    }
    if context.editable {
        groups.push(vec![
            (Cut, context.can_cut),
            (Copy, context.can_copy),
            (Paste, context.can_paste),
            (SelectAll, true),
        ]);
    } else if !context.selection.is_empty() {
        groups.push(vec![(Copy, true), (SelectAll, true)]);
    }
    groups.push(vec![
        (Back, context.can_back),
        (Forward, context.can_forward),
        (Reload, true),
    ]);
    groups.push(vec![
        (Find, true),
        (SavePage, true),
        (Print, true),
        (OpenExternal, true),
    ]);
    groups.push(vec![(Inspect, true)]);
    groups.push(
        BrowserCommand::CAPTURES
            .iter()
            .map(|command| (*command, true))
            .collect(),
    );
    for (at, group) in groups.into_iter().enumerate() {
        if at > 0 {
            menu = menu.separator();
        }
        for (command, on) in group {
            menu = match command {
                Suggestion(index) => {
                    let word = context
                        .suggestions
                        .get(usize::from(index))
                        .map_or("", String::as_str);
                    menu.item(word, None, command.id(), false)
                }
                _ => row(menu, command, on),
            };
        }
    }
    menu
}

/// The menu of what can be put into the prompt from the page, to emit in the box of the
/// button of the bar that opens it and hang from it, while it is open there.
pub fn capture_dropdown(state: &WorkspaceState) -> Option<Menu> {
    state
        .browser_menu
        .filter(|open| open.at.is_none())
        .map(|open| browser_menu_rows(open, Menu::dropdown()))
}

/// `menu` holding the rows of the menu over a page that `open` says it offers.
fn browser_menu_rows(open: BrowserMenu, menu: Menu) -> Menu {
    let row = |menu: Menu, command: BrowserCommand| {
        menu.item(command.label(), command.keys(), command.id(), false)
    };
    let operations = crate::events::operations();
    let menu = menu.width(68.0).row_height(6.5).on_pick(move |row| {
        if let Some(command) = BrowserCommand::of(row) {
            operations.browser_menu_command(command);
        }
    });
    if open.captures_only {
        return BrowserCommand::CAPTURES.into_iter().fold(menu, row);
    }
    use BrowserCommand::*;
    let groups: [&[BrowserCommand]; 7] = [
        &[Back, Forward, Reload, HardReload],
        &[Cut, Copy, Paste, SelectAll],
        &[Find],
        &[ZoomIn, ZoomOut, ZoomReset],
        &[SavePage, Print, OpenExternal, ClearData],
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
}
