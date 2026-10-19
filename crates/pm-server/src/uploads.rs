//! The files clients attach to prompts, kept where an agent can read them.

use std::io::Write;
use std::path::PathBuf;

use pm_protocol::Attachment;

/// The folder under the app's data folder that attached files are kept in.
const FOLDER: &str = "uploads";

/// Writes `attachments` for `session` under the data folder and returns where each went.
pub(crate) fn store(session: u64, attachments: &[Attachment]) -> Vec<PathBuf> {
    let Some(root) = pm_store::data_directory() else {
        return Vec::new();
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    attachments
        .iter()
        .enumerate()
        .filter_map(|(position, attachment)| {
            let name = std::path::Path::new(&attachment.name)
                .file_name()?
                .to_string_lossy()
                .into_owned();
            let folder = root
                .join(FOLDER)
                .join(session.to_string())
                .join(format!("{stamp:x}-{position}"));
            pm_store::create_private_directory(&folder).ok()?;
            let path = folder.join(name);
            pm_store::open_private(&path, false)
                .ok()?
                .write_all(&attachment.data)
                .ok()?;
            Some(path)
        })
        .collect()
}

/// Deletes uploads belonging to a discarded prompt and their individual folders.
pub(crate) fn discard(paths: &[String]) {
    for path in paths {
        let path = std::path::Path::new(path);
        if let Err(error) = std::fs::remove_file(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("could not remove upload {}: {error}", path.display());
        }
        if let Some(folder) = path.parent() {
            let _ = std::fs::remove_dir(folder);
        }
    }
}
