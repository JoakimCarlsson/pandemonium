//! The keys a page of the browser takes for the browser itself, as a browser's own do, instead of passing them to the page.

use pm_input::{Key, KeyEvent};

use crate::browser::{Command, ZoomStep};
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
}

/// What `event` does in the browser while a page has the keyboard, if it is a key the browser takes: with Control or Command, `F` finds, `G` steps through the matches, `=`, `-` and `0` zoom, `R` reloads and with Shift reloads without the cache, `T` opens a new tab and `W` closes the tab; and `F3`, `F5` and Control `F5` do the same on their own.
pub fn shortcut_of(event: &KeyEvent) -> Option<Shortcut> {
    let modifiers = event.modifiers;
    let command = (modifiers.control || modifiers.logo) && !modifiers.alt;
    match event.key {
        Key::Character(character) if command => match character.to_ascii_lowercase() {
            'f' => Some(Shortcut::Find),
            'g' if modifiers.shift => Some(Shortcut::FindPrevious),
            'g' => Some(Shortcut::FindNext),
            '=' | '+' => Some(Shortcut::ZoomIn),
            '-' | '_' => Some(Shortcut::ZoomOut),
            '0' => Some(Shortcut::ZoomReset),
            'r' if modifiers.shift => Some(Shortcut::HardReload),
            'r' => Some(Shortcut::Reload),
            't' => Some(Shortcut::NewTab),
            'w' => Some(Shortcut::CloseTab),
            _ => None,
        },
        Key::Function(3) if !modifiers.alt && !command => Some(if modifiers.shift {
            Shortcut::FindPrevious
        } else {
            Shortcut::FindNext
        }),
        Key::Function(5) if !modifiers.alt => Some(if command || modifiers.shift {
            Shortcut::HardReload
        } else {
            Shortcut::Reload
        }),
        _ => None,
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
