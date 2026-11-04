//! The conversation of a session as the harness saved it, read and named without an agent running.

use std::path::Path;

use omc_wire::{Conversation, Imported};

/// The conversation saved under `id` in `folder`: its name, the choices made about it and its turns, oldest first; an empty one when nothing was saved.
///
/// # Errors
///
/// Fails with the reason when the saved conversation cannot be read.
pub fn saved_conversation(id: &str, folder: &Path) -> Result<Conversation, String> {
    omc_session::history(id, Some(folder.to_path_buf()))
}

/// Names the conversation saved under `id` in `folder` `title`, for a conversation no agent has open.
///
/// # Errors
///
/// Fails with the reason when the name cannot be written, as when an agent has the conversation open.
pub fn rename_saved(id: &str, folder: &Path, title: &str) -> Result<(), String> {
    omc_session::rename(id, Some(folder.to_path_buf()), title)
}

/// Hands the harness the conversation `imported`, which a client kept itself, to save under `id` in `folder` as its own.
///
/// # Errors
///
/// Fails with the reason when the harness refuses, as when it already holds a history for `id`.
pub fn import_saved(id: &str, folder: &Path, imported: &Imported) -> Result<(), String> {
    omc_session::import(id, Some(folder.to_path_buf()), imported)
}

#[cfg(test)]
/// Tests that read and write saved conversations.
mod tests {
    use super::{rename_saved, saved_conversation};

    /// Nothing saved reads as an empty conversation.
    #[test]
    fn an_empty_history_reads_as_empty() {
        let folder =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/history-live");
        let saved = saved_conversation("nothing-here", &folder).unwrap();
        assert!(saved.turns.is_empty() && saved.title.is_none());
    }

    /// A conversation named while no agent has it open comes back with its name.
    #[test]
    fn a_saved_conversation_can_be_named() {
        let folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/history-live-rename");
        let _ = std::fs::remove_dir_all(&folder);
        rename_saved("named", &folder, "Fix the thing").unwrap();
        let saved = saved_conversation("named", &folder).unwrap();
        assert_eq!(saved.title.as_deref(), Some("Fix the thing"));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
