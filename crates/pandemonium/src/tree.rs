//! The menu of things that can be done to one entry of the file tree.
//!
//! The tree is a view of the disk, so its menu is the handful of things one
//! does to a file as a file: make another beside it, rename it, take it off
//! the disk, say where it is. What is inside a file is the editor's business
//! and is not offered here.

use pm_core::{Entry, EntryId};
use pm_ui::{MenuItem, menu_entry, menu_separator};

use crate::message::Message;

/// The things that can be done to `entry`.
pub fn entry_menu(entry: &Entry) -> Vec<MenuItem<Message>> {
    let id = entry.id();
    let inside = if entry.is_directory() {
        "Inside"
    } else {
        "Here"
    };

    vec![
        menu_entry(format!("New File {inside}…"), Some(Message::NewFileIn(id))),
        menu_entry(
            format!("New Folder {inside}…"),
            Some(Message::NewFolderIn(id)),
        ),
        menu_separator(),
        menu_entry("Rename…", Some(Message::RenameEntry(id))),
        menu_entry("Delete…", Some(Message::DeleteEntry(id))),
        menu_separator(),
        menu_entry("Copy Path", Some(Message::CopyEntryPath(id))),
        menu_entry(
            "Copy Relative Path",
            Some(Message::CopyEntryRelativePath(id)),
        ),
        menu_separator(),
        menu_entry("Reveal in File Manager", Some(Message::RevealEntry(id))),
        menu_entry("Open in Terminal", Some(Message::OpenEntryInTerminal(id))),
    ]
}

/// The entry `id` names in `tree`, if the tree still holds it.
pub fn entry_of(tree: &pm_core::FileTree, id: EntryId) -> Option<&Entry> {
    tree.rows()
        .into_iter()
        .map(|row| row.entry)
        .find(|entry| entry.id() == id)
}
