//! Pasting files and pictures from the clipboard into the prompt as attachments.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use pm_pages::PROMPT;

use super::App;

/// The folder under the data directory that pasted pictures are saved in.
const PASTED: &str = "pasted";

impl App {
    /// Attaches the files or the picture on the clipboard to the prompt, and
    /// reports whether there were any; text is left for the caller to paste.
    pub(super) fn paste_attachments(&mut self) -> bool {
        let Some(clipboard) = self.clipboard() else {
            return false;
        };
        let files = clipboard
            .get()
            .file_list()
            .ok()
            .filter(|files| !files.is_empty());
        let paths: Vec<String> = match files {
            Some(files) => files
                .iter()
                .map(|file| file.to_string_lossy().into_owned())
                .collect(),
            None => match clipboard.get_image().ok().and_then(save_picture) {
                Some(path) => vec![path.to_string_lossy().into_owned()],
                None => return false,
            },
        };
        self.attach_paths(paths);
        true
    }

    /// Attaches the files at `paths` to the prompt, starts their previews and focuses the prompt.
    pub(super) fn attach_paths(&mut self, paths: Vec<String>) {
        self.model
            .update(|state| *state = std::mem::take(state).with_attachments(&paths));
        self.work.preview(paths);
        self.focus.focus(PROMPT);
    }
}

/// Saves `picture` as a PNG in the pasted folder of the data directory and returns where.
fn save_picture(picture: arboard::ImageData<'_>) -> Option<PathBuf> {
    let width = u32::try_from(picture.width).ok()?;
    let height = u32::try_from(picture.height).ok()?;
    let pixels = image::RgbaImage::from_raw(width, height, picture.bytes.into_owned())?;
    let folder = pm_store::data_directory()?.join(PASTED);
    std::fs::create_dir_all(&folder).ok()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    let path = folder.join(format!("pasted-{stamp}.png"));
    pixels.save(&path).ok()?;
    Some(path)
}
