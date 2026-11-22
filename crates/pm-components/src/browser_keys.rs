//! The keys a page of the browser takes for the browser itself, as a browser's own do, instead of passing them to the page.

use rustle::{Key, KeyEvent};

use crate::browser::{Command, ZoomStep};
use crate::browser_chrome::ChromeAction;
use crate::state::WorkspaceState;

/// What a key of the browser does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shortcut {
    /// Shows the bar for finding in the page.
    Find,
    /// Goes to the next match.
    FindNext,
    /// Goes to the previous match.
    FindPrevious,
    /// Zooms the page in one step.
    ZoomIn,
    /// Zooms the page out one step.
    ZoomOut,
    /// Puts the zoom back to 100%.
    ZoomReset,
    /// Loads the page again.
    Reload,
    /// Loads the page again without the cache.
    HardReload,
    /// Opens a new tab.
    NewTab,
    /// Closes the tab being shown.
    CloseTab,
    /// Puts the keyboard in the address bar.
    FocusAddress,
    /// Goes back one page in the history.
    Back,
    /// Goes forward one page in the history.
    Forward,
    /// Shows the next tab, going round after the last.
    NextTab,
    /// Shows the previous tab, going round before the first.
    PreviousTab,
    /// Opens again the tab closed last.
    ReopenTab,
    /// Shows or hides the developer tools.
    DevTools,
    /// Prints the page to a PDF and opens it.
    Print,
    /// Stops the page loading.
    Stop,
    /// Takes the page out of full screen.
    LeaveFullscreen,
}

/// What `event` does in the browser while a page has the keyboard, if it is a key the browser takes: with Control or Command, `F` finds, `G` steps through the matches, `=`, `-` and `0` zoom, `R` reloads and with Shift reloads without the cache, `T` opens a new tab and with Shift opens again the tab closed last, `W` closes the tab, `L` goes to the address bar, `P` prints, Shift `I` shows the developer tools, `[` and `]` go back and forward, Tab and Page Down show the next tab and Shift Tab and Page Up the previous one; Alt with the left and right arrows goes back and forward; and `F3`, `F5`, Control `F5`, `F6` and `F12` do the same on their own.
pub fn shortcut_of(event: &KeyEvent) -> Option<Shortcut> {
    let modifiers = event.modifiers;
    let command = (modifiers.ctrl || modifiers.meta) && !modifiers.alt;
    let control = modifiers.ctrl && !modifiers.alt && !modifiers.meta;
    let bare = !modifiers.ctrl && !modifiers.meta && !modifiers.alt;
    match &event.key {
        Key::Character(_) if command => match event.code_point()?.to_ascii_lowercase() {
            'f' => Some(Shortcut::Find),
            'g' if modifiers.shift => Some(Shortcut::FindPrevious),
            'g' => Some(Shortcut::FindNext),
            '=' | '+' => Some(Shortcut::ZoomIn),
            '-' | '_' => Some(Shortcut::ZoomOut),
            '0' => Some(Shortcut::ZoomReset),
            'r' if modifiers.shift => Some(Shortcut::HardReload),
            'r' => Some(Shortcut::Reload),
            't' if modifiers.shift => Some(Shortcut::ReopenTab),
            't' => Some(Shortcut::NewTab),
            'w' => Some(Shortcut::CloseTab),
            'l' => Some(Shortcut::FocusAddress),
            'p' if !modifiers.shift => Some(Shortcut::Print),
            'i' if modifiers.shift => Some(Shortcut::DevTools),
            '[' | '{' if modifiers.meta => Some(Shortcut::Back),
            ']' | '}' if modifiers.meta => Some(Shortcut::Forward),
            _ => None,
        },
        Key::Tab if control => Some(if modifiers.shift {
            Shortcut::PreviousTab
        } else {
            Shortcut::NextTab
        }),
        Key::PageDown if control && !modifiers.shift => Some(Shortcut::NextTab),
        Key::PageUp if control && !modifiers.shift => Some(Shortcut::PreviousTab),
        Key::ArrowLeft if modifiers.alt && !modifiers.ctrl && !modifiers.meta => {
            Some(Shortcut::Back)
        }
        Key::ArrowRight if modifiers.alt && !modifiers.ctrl && !modifiers.meta => {
            Some(Shortcut::Forward)
        }
        Key::F(3) if !modifiers.alt && !command => Some(if modifiers.shift {
            Shortcut::FindPrevious
        } else {
            Shortcut::FindNext
        }),
        Key::F(5) if !modifiers.alt => Some(if command || modifiers.shift {
            Shortcut::HardReload
        } else {
            Shortcut::Reload
        }),
        Key::F(6) if bare && !modifiers.shift => Some(Shortcut::FocusAddress),
        Key::F(12) if bare && !modifiers.shift => Some(Shortcut::DevTools),
        _ => None,
    }
}

/// What Escape does in the browser while a page has the keyboard and `state` is as it is: it takes the page out of full screen, or stops it loading; otherwise it is the page's.
pub fn escape_of(event: &KeyEvent, state: &WorkspaceState) -> Option<Shortcut> {
    let modifiers = event.modifiers;
    if !matches!(event.key, Key::Escape)
        || modifiers.ctrl
        || modifiers.meta
        || modifiers.alt
        || modifiers.shift
    {
        return None;
    }
    let browser = state.browser.as_ref()?;
    if browser.chrome.fullscreen {
        Some(Shortcut::LeaveFullscreen)
    } else if browser.life.loading {
        Some(Shortcut::Stop)
    } else {
        None
    }
}

impl WorkspaceState {
    /// Carries out `shortcut` on the browser.
    pub fn browser_shortcut(&mut self, shortcut: Shortcut) {
        match shortcut {
            Shortcut::Find => self.browser_find_opened(),
            Shortcut::FindNext => self.browser_find_stepped(false),
            Shortcut::FindPrevious => self.browser_find_stepped(true),
            Shortcut::ZoomIn => self.browser_queue(Command::Zoom(ZoomStep::In)),
            Shortcut::ZoomOut => self.browser_queue(Command::Zoom(ZoomStep::Out)),
            Shortcut::ZoomReset => self.browser_queue(Command::Zoom(ZoomStep::Reset)),
            Shortcut::Reload => self.browser_queue(Command::Reload),
            Shortcut::HardReload => self.browser_queue(Command::HardReload),
            Shortcut::NewTab => self.browser_new_tab(),
            Shortcut::CloseTab => self.browser_close_tab(),
            Shortcut::FocusAddress => {}
            Shortcut::Back => self.browser_queue(Command::Back),
            Shortcut::Forward => self.browser_queue(Command::Forward),
            Shortcut::NextTab | Shortcut::PreviousTab => {
                if let Some(browser) = self.browser.as_mut() {
                    browser.select_step(if shortcut == Shortcut::NextTab { 1 } else { -1 });
                }
            }
            Shortcut::ReopenTab => {
                if let Some(browser) = self.browser.as_mut() {
                    browser.reopen_closed();
                }
            }
            Shortcut::DevTools => {
                if let Some(browser) = self.browser.as_mut() {
                    browser.tools = !browser.tools;
                    let open = browser.tools;
                    browser.commands.push(if open {
                        Command::OpenTools
                    } else {
                        Command::CloseTools
                    });
                }
            }
            Shortcut::Print => self.browser_queue(Command::Chrome(ChromeAction::Print)),
            Shortcut::Stop => self.browser_queue(Command::Stop),
            Shortcut::LeaveFullscreen => self.browser_leave_fullscreen(),
        }
    }

    /// Queues `command` for the engine.
    pub(crate) fn browser_queue(&mut self, command: Command) {
        if let Some(browser) = self.browser.as_mut() {
            browser.commands.push(command);
        }
    }
}

#[cfg(test)]
mod tests;
