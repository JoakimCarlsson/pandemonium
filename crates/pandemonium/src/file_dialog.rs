//! The desktop's own dialog for picking files to attach to a prompt; the
//! files are read on this machine when the prompt is sent, so the dialog is
//! this machine's whichever machine the session runs on.

/// Asks the desktop for files to attach, returning their paths, or none when the dialog is dismissed.
pub fn pick_attachments() -> Vec<String> {
    rfd::FileDialog::new()
        .set_title("Attach files")
        .pick_files()
        .unwrap_or_default()
        .iter()
        .map(|file| file.to_string_lossy().into_owned())
        .collect()
}
