//! The menus of the file tree: one for what is selected, one for the tree.
//!
//! The tree is a view of the disk, so its menu is the handful of things one
//! does to a file as a file: make another beside it, rename it, move it, take
//! it off the disk, say where it is. What is inside a file is the editor's
//! business and is not offered here. Like the list of changes, the menu is
//! about everything selected, and what each entry says counts it.

use pm_ui::{MenuItem, menu_entry, menu_separator};

use crate::message::Message;

/// The things that can be done to the `count` rows selected.
///
/// `directory` says whether the row the menu was opened on holds others,
/// and `pasting` whether there is anything on the tree's clipboard.
pub fn entry_menu(count: usize, directory: bool, pasting: bool) -> Vec<MenuItem<Message>> {
    let alone = count == 1;
    let only = |message: Message| alone.then_some(message);
    let counted = |label: &str| match alone {
        true => label.to_owned(),
        false => format!("{label} {count} Items"),
    };

    vec![
        menu_entry("New File…", Some(Message::NewTreeFile)),
        menu_entry("New Folder…", Some(Message::NewTreeFolder)),
        menu_separator(),
        menu_entry(
            "Open to the Side",
            (!directory).then_some(Message::OpenTreeEntriesToSide),
        ),
        menu_entry("Reveal in File Manager", only(Message::RevealTreeEntry)),
        menu_entry("Open in Terminal", only(Message::OpenTreeEntryInTerminal)),
        menu_separator(),
        menu_entry(counted("Cut"), Some(Message::CutTreeEntries)),
        menu_entry(counted("Copy"), Some(Message::CopyTreeEntries)),
        menu_entry("Paste", pasting.then_some(Message::PasteTreeEntries)),
        menu_entry(counted("Duplicate"), Some(Message::DuplicateTreeEntries)),
        menu_separator(),
        menu_entry("Copy Path", Some(Message::CopyTreePaths)),
        menu_entry("Copy Relative Path", Some(Message::CopyTreeRelativePaths)),
        menu_separator(),
        menu_entry("Rename…", only(Message::RenameTreeEntry)),
        menu_entry(counted("Delete"), Some(Message::TrashTreeEntries)),
        menu_entry(
            counted("Delete Permanently"),
            Some(Message::DeleteTreeEntries),
        ),
    ]
}

/// The things that can be done to the tree itself, from the space below it.
pub fn empty_menu(pasting: bool) -> Vec<MenuItem<Message>> {
    vec![
        menu_entry("New File…", Some(Message::NewTreeFile)),
        menu_entry("New Folder…", Some(Message::NewTreeFolder)),
        menu_separator(),
        menu_entry("Paste", pasting.then_some(Message::PasteTreeEntries)),
        menu_separator(),
        menu_entry("Reveal in File Manager", Some(Message::RevealTreeEntry)),
        menu_entry("Open in Terminal", Some(Message::OpenTreeEntryInTerminal)),
        menu_separator(),
        menu_entry("Collapse All", Some(Message::CollapseTree)),
        menu_entry("Refresh", Some(Message::RefreshTree)),
    ]
}
