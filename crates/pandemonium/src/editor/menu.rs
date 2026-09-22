//! The menu of things that can be done to the text under the pointer.
//!
//! Every entry resolves to the [`Action`] a keybinding would, so the menu is
//! a second way of asking for the same commands rather than a second set of
//! them. An entry that does not apply where it was opened — pasting nothing,
//! renaming in a file no server is watching — is greyed rather than left
//! out, so the menu keeps its shape wherever it is opened.

use pm_ui::{MenuItem, menu_entry, menu_separator};

use crate::editor::FileId;
use crate::keymap::Action;
use crate::message::Message;
use crate::panes::PaneId;

/// What the pointer was over when the menu was asked for.
pub struct TextMenu {
    /// The pane holding the text.
    pub pane: PaneId,
    /// The file being shown in it.
    pub file: FileId,
    /// Whether anything is selected.
    pub selected: bool,
    /// Whether a language server is watching the file.
    pub served: bool,
    /// Whether the file is in a repository the editor can read.
    pub tracked: bool,
}

/// The things that can be done to the text, given what is under the pointer.
pub fn text_menu(target: &TextMenu) -> Vec<MenuItem<Message>> {
    let TextMenu {
        pane,
        file,
        selected,
        served,
        tracked,
    } = *target;
    let language = |action: Action| served.then_some(Message::PaneAction(pane, action));
    let always = |action: Action| Some(Message::PaneAction(pane, action));

    vec![
        menu_entry("Go to Definition", language(Action::GoToDefinition)),
        menu_entry(
            "Go to Type Definition",
            language(Action::GoToTypeDefinition),
        ),
        menu_entry("Go to Implementation", language(Action::GoToImplementation)),
        menu_entry("Find All References", language(Action::FindReferences)),
        menu_separator(),
        menu_entry("Rename Symbol", language(Action::Rename)),
        menu_entry("Format Document", language(Action::Format)),
        menu_entry("Code Actions", language(Action::ShowCodeActions)),
        menu_separator(),
        menu_entry("Cut", always(Action::Cut)),
        menu_entry("Copy", always(Action::Copy)),
        menu_entry("Paste", always(Action::Paste)),
        menu_separator(),
        menu_entry("Toggle Comment", always(Action::ToggleComment)),
        menu_entry("Duplicate Line", always(Action::DuplicateLine)),
        menu_entry(
            "Find Selection",
            selected.then_some(Message::PaneAction(pane, Action::FindSelection)),
        ),
        menu_separator(),
        menu_entry(
            "Toggle Git Blame",
            tracked.then_some(Message::PaneAction(pane, Action::ToggleBlame)),
        ),
        menu_separator(),
        menu_entry("Copy Path", Some(Message::CopyFilePath(file))),
        menu_entry(
            "Copy Relative Path",
            Some(Message::CopyFileRelativePath(file)),
        ),
        menu_entry("Reveal in File Manager", Some(Message::RevealFile(file))),
    ]
}
