//! The scratch folders of chats: sessions that belong to no project, each working in a folder of its own under the app's data folder.

use std::path::{Path, PathBuf};

/// The folder under the app's data folder that the scratch folders of chats are kept in.
const FOLDER: &str = "chats";

/// The folder every chat's scratch folder is in.
fn root() -> Option<PathBuf> {
    pm_store::data_directory().map(|root| root.join(FOLDER))
}

/// Makes the scratch folder of the chat numbered `number`, which only the reader can open, and returns it.
///
/// # Errors
///
/// Fails when the app has no data folder, when the folder is there already, so that a new chat never takes over what another left, or when it cannot be made.
pub(crate) fn create(number: u64) -> Result<PathBuf, String> {
    let folder = root()
        .ok_or_else(|| "the app has no data folder".to_owned())?
        .join(number.to_string());
    if folder.exists() {
        return Err(format!("{} is there already", folder.display()));
    }
    pm_store::create_private_directory(&folder).map_err(|error| error.to_string())?;
    Ok(folder)
}

/// Whether `folder` is the scratch folder of a chat: a folder directly inside the one chats are kept in, named by a chat's number.
pub(crate) fn owns(folder: &Path) -> bool {
    let numbered = folder
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.parse::<u64>().is_ok());
    numbered && root().is_some_and(|root| folder.parent() == Some(root.as_path()))
}

/// Removes the scratch folder `folder` of a chat with everything in it; nothing happens when it is gone already.
///
/// # Errors
///
/// Fails when `folder` is not the scratch folder of a chat, so that nothing else is ever deleted, or when it cannot be removed.
pub(crate) fn remove(folder: &Path) -> Result<(), String> {
    if !owns(folder) {
        return Err(format!("{} is not a chat's folder", folder.display()));
    }
    match std::fs::remove_dir_all(folder) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}
